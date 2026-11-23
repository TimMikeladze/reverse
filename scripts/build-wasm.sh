#!/usr/bin/env bash
# Build the WebAssembly module and the self-contained playground page.
#
#   scripts/build-wasm.sh
#
# Needs the wasm32-unknown-unknown target:  rustup target add wasm32-unknown-unknown
# Without rustup access, a toolchain with the standard library sources can
# build it instead: set REVERSE_BUILD_STD=1.
set -euo pipefail
cd "$(dirname "$0")/.."

export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-zstack-size=33554432"
if [ "${REVERSE_BUILD_STD:-0}" = "1" ]; then
    RUSTC_BOOTSTRAP=1 cargo build --profile wasm -p reverse-wasm --target wasm32-unknown-unknown -Zbuild-std=std,panic_abort
else
    cargo build --profile wasm -p reverse-wasm --target wasm32-unknown-unknown
fi

wasm=target/wasm32-unknown-unknown/wasm/reverse_wasm.wasm
mkdir -p playground/dist
cp "$wasm" playground/dist/reverse.wasm
python3 playground/bundle.py
ls -la playground/dist
