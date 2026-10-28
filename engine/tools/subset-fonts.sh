#!/usr/bin/env bash
# Subsets the embedded fonts (D-023): Latin, Greek, Cyrillic, punctuation, arrows, maths, boxes,
# shapes and dingbats, with the OpenType features a default shaping uses (kerning, ligatures,
# marks). Text measurement is unchanged for these characters; the WebAssembly module loses 2.6 MB.
# Needs fontTools (pip install fonttools). Run on the original fonts (the subset of a subset is the
# same subset).
set -euo pipefail
cd "$(dirname "$0")/../crates/engine/assets/fonts"
U="U+0000-024F,U+0300-036F,U+0370-03FF,U+0400-04FF,U+1E00-1EFF,U+2000-206F,U+20A0-20CF,U+2100-214F,U+2190-21FF,U+2200-22FF,U+2300-23FF,U+2460-24FF,U+2500-257F,U+25A0-25FF,U+2600-26FF,U+2700-27BF,U+FB00-FB06,U+FFFD"
FEAT="abvm,blwm,ccmp,locl,mark,mkmk,rlig,calt,clig,curs,dist,kern,liga,rclt"
for f in *.otf *.ttf; do
  pyftsubset "$f" --unicodes="$U" --layout-features="$FEAT" --no-hinting --output-file="$f.sub"
  mv "$f.sub" "$f"
done
ls -l *.otf *.ttf
