#!/bin/sh
# Builds the WebAssembly example and serves it on http://localhost:8765/
# Needs: the wasm32-unknown-unknown target, wasm-bindgen-cli 0.2.129.
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/router_wasm_demo.wasm
rm -rf fixtures && cp -r ../../fixtures fixtures
ls -l pkg/router_wasm_demo_bg.wasm
[ "$1" = "--serve" ] && exec python3 -m http.server 8765
exit 0
