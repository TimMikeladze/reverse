#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-} -C link-arg=-zstack-size=8388608"
cargo build --locked --profile browser --target wasm32-unknown-unknown

output=target/browser-dist
mkdir -p "$output/assets"
"${WASM_BINDGEN:-wasm-bindgen}" --target web --no-typescript --remove-name-section \
    --out-name observatory --out-dir "$output" target/wasm32-unknown-unknown/browser/reverse-bevy-world.wasm
cp web/index.html web/editor.js "$output/"
cp assets/*.verse "$output/assets/"
