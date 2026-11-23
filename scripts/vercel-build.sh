#!/usr/bin/env bash
# Build everything the website serves into dist/:
#
#   dist/index.html              landing page and documentation (site/build.py)
#   dist/playground/index.html   the browser playground
#   dist/playground/reverse.wasm the toolchain compiled to WebAssembly
#
# Vercel runs this on every push (see vercel.json); pushes to main go to
# production. It also works locally: scripts/vercel-build.sh && open dist/index.html
set -euo pipefail
cd "$(dirname "$0")/.."

# Rust with the WebAssembly target. Vercel's build image has no Rust, so
# install a minimal toolchain there; locally, use the one already present.
if ! command -v cargo >/dev/null 2>&1; then
    if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
            sh -s -- -y --profile minimal --default-toolchain stable --target wasm32-unknown-unknown
    fi
    export PATH="$HOME/.cargo/bin:$PATH"
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
python3 site/build.py

rm -rf dist
mkdir -p dist/playground
cp site/dist/index.html dist/index.html
cp playground/dist/index.html playground/dist/reverse.wasm dist/playground/
ls -la dist dist/playground
