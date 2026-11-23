#!/usr/bin/env bash
# Build everything the website serves into dist/:
#
#   dist/index.html              landing page and documentation (site/build.py)
#   dist/playground/index.html   the browser playground
#   dist/playground/reverse.wasm the toolchain compiled to WebAssembly
#   dist/observatory/            the Bevy world and its Verse editor
#
# Vercel runs this on every push (see vercel.json); pushes to main go to
# production. It also works locally: scripts/vercel-build.sh && open dist/index.html
set -euo pipefail
cd "$(dirname "$0")/.."

world=examples/bevy-world
minimum_rust=$(awk -F '"' '/^rust-version = / { print $2; exit }' "$world/Cargo.toml")
current_rust=$(rustc --version 2>/dev/null | awk '{ print $2 }' || true)
if ! awk -v current="$current_rust" -v minimum="$minimum_rust" 'BEGIN {
    split(current, installed, "."); split(minimum, required, ".");
    for (i = 1; i <= 3; i++) {
        if (installed[i] + 0 != required[i] + 0) exit !(installed[i] + 0 > required[i] + 0);
    }
}'; then
    cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$cargo_bin/rustup" ]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
            sh -s -- -y --profile minimal --default-toolchain none
    fi
    export PATH="$cargo_bin:$PATH"
    rustup toolchain install "$minimum_rust" --profile minimal --target wasm32-unknown-unknown
    export RUSTUP_TOOLCHAIN="$minimum_rust"
    toolchain_bin=$(dirname "$(rustup which --toolchain "$minimum_rust" cargo)")
    export PATH="$toolchain_bin:$PATH"
fi
if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 ||
        echo "note: could not add the wasm32 target with rustup; trying the build anyway"
fi

# Python Markdown for the documentation pages.
if ! python3 -c "import markdown" 2>/dev/null; then
    python3 -m pip install --quiet markdown 2>/dev/null ||
        python3 -m pip install --quiet --user markdown 2>/dev/null ||
        python3 -m pip install --quiet --break-system-packages markdown
fi

scripts/build-wasm.sh

bindgen_version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' "$world/Cargo.lock")
bindgen_cli=${WASM_BINDGEN:-wasm-bindgen}
if [ "$("$bindgen_cli" --version 2>/dev/null || true)" != "wasm-bindgen $bindgen_version" ]; then
    tools="$PWD/$world/target/browser-tools/$bindgen_version"
    bindgen_cli="$tools/wasm-bindgen"
    if [ ! -x "$bindgen_cli" ]; then
        mkdir -p "$tools"
        case "$(uname -sm)" in
            "Linux x86_64") platform=x86_64-unknown-linux-musl ;;
            "Darwin arm64") platform=aarch64-apple-darwin ;;
            "Darwin x86_64") platform=x86_64-apple-darwin ;;
            *) platform= ;;
        esac
        if [ -n "$platform" ]; then
            release="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$bindgen_version"
            curl -fsSL "$release/wasm-bindgen-$bindgen_version-$platform.tar.gz" |
                tar -xz -C "$tools" --strip-components=1 wasm-bindgen-"$bindgen_version"-"$platform"/wasm-bindgen
        else
            cargo install --locked --version "$bindgen_version" --root "$tools" wasm-bindgen-cli
            bindgen_cli="$tools/bin/wasm-bindgen"
        fi
    fi
fi
WASM_BINDGEN="$bindgen_cli" bash "$world/build-browser.sh"
python3 site/build.py

rm -rf dist
mkdir -p dist/playground dist/observatory
cp site/dist/index.html dist/index.html
cp playground/dist/index.html playground/dist/reverse.wasm dist/playground/
cp -R "$world/target/browser-dist/." dist/observatory/
ls -la dist dist/playground dist/observatory
