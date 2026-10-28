#!/bin/sh
# F-WASM-2: WebAssembly produces the same layout.json as native, on every example.
# Requirements: wasm32-unknown-unknown target, wasm-bindgen-cli (same version as the crate), node.
# Usage: tools/wasm-parity.sh   (from the engine folder)
set -e
# the target folder: the engine's own, or the enclosing workspace's
TARGET=$(cargo metadata -q --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
OUT=$TARGET/wasm-parity
cargo build -q --profile wasm -p skemo-engine-wasm --target wasm32-unknown-unknown
wasm-bindgen --target nodejs --out-dir "$OUT/pkg" "$TARGET/wasm32-unknown-unknown/wasm/skemo_engine_wasm.wasm"
cargo build -q --release -p skemo-engine --example layout
cat > "$OUT/run.js" <<'JS'
const { render } = require('./pkg/skemo_engine_wasm.js');
const fs = require('fs');
const [src, css] = process.argv.slice(2);
const r = JSON.parse(render(fs.readFileSync(src, 'utf8'), css ? fs.readFileSync(css, 'utf8') : '', '{}'));
process.stdout.write(JSON.stringify(r.layout, null, 2) + '\n');
JS
fail=0
for c in "reseau.pug" "reseau-manuel.pug" "reseau-manuel-via.pug" "reseau-2pins.pug" "reseau-conflit.pug" "reseau-entete.pug" \
         "process.pug process.css" "design.pug design.css" "animation.pug" "timeline.pug timeline.css" "frise.pug frise.css"; do
  set -- $c
  # comparison after JSON normalisation (JavaScript rewrites 177.0 as 177)
  native=$("$TARGET/release/examples/layout" "fixtures/$1" ${2:+fixtures/$2} 2>/dev/null | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.stringify(JSON.parse(s))))')
  wasm=$(node "$OUT/run.js" "fixtures/$1" ${2:+fixtures/$2} | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.stringify(JSON.parse(s))))')
  if [ "$native" = "$wasm" ]; then echo "identical: $1"; else echo "DIFFERENT: $1"; fail=1; fi
done
ls -l "$OUT/pkg/skemo_engine_wasm_bg.wasm" | awk '{print "wasm size: " $5 " bytes"}'
gzip -9 -c "$OUT/pkg/skemo_engine_wasm_bg.wasm" | wc -c | awk '{print "gzip size: " $1 " bytes"}'
exit $fail
