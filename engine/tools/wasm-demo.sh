#!/bin/sh
# Builds the WebAssembly demo: crates/engine-wasm/demo/ (serve it over HTTP: python3 -m http.server)
set -e
cargo build -q --profile wasm -p skemo-engine-wasm --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir crates/engine-wasm/demo/pkg target/wasm32-unknown-unknown/wasm/skemo_engine_wasm.wasm
echo "demo ready: cd crates/engine-wasm/demo && python3 -m http.server 8000"
