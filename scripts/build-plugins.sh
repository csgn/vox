#!/usr/bin/env bash
# Build the plugins in plugins/ for the WebAssembly target.
# Output: plugins/target/wasm32-wasip2/release/<name>.wasm
# Needs: rustup target add wasm32-wasip2
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --manifest-path plugins/Cargo.toml --target wasm32-wasip2 --release "$@"
ls -1 plugins/target/wasm32-wasip2/release/*.wasm
