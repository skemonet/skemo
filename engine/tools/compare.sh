#!/bin/sh
# compare.sh <skemo before> <skemo after> <out dir> [--css=FILE] [files or folders…]
# The layout method, phase 0 (meta repository, docs/plans/layout-method.md): renders every diagram
# with both binaries (default: the repository's corpus, as tests/ledger.rs reads it) and writes, for
# each one whose image differs, a sheet "before | after" in <out dir>/sheets. Needs ImageMagick.
set -eu
A=$1; B=$2; O=$3; shift 3
CSS=""
case "${1:-}" in --css=*) CSS="--css ${1#--css=}"; shift ;; esac
R=$(cd "$(dirname "$0")/../.." && pwd)
if [ $# -eq 0 ]; then
  set -- "$R/engine/fixtures" "$R/engine/crates/engine/tests/data" "$R/site/gallery" "$R/cli/examples/site" \
    $(ls "$R"/lang/site/examples/*.skemo "$R"/site/examples/*.skemo)
fi
mkdir -p "$O/before" "$O/after" "$O/sheets"
rm -f "$O"/sheets/*.png
find "$@" \( -name '*.skemo' -o -name '*.pug' \) | sort | while read -r f; do
  n=$(echo "$f" | sed "s#^$R/##; s#/#_#g; s#\.[a-z]*\$##")
  own="${f%.*}.css"; extra=""
  [ -f "$own" ] && extra="--css $own"
  # shellcheck disable=SC2086
  "$A" render "$f" -o "$O/before/$n.png" -q $CSS $extra 2>/dev/null || true
  # shellcheck disable=SC2086
  "$B" render "$f" -o "$O/after/$n.png" -q $CSS $extra 2>/dev/null || true
  if [ -f "$O/before/$n.png" ] && [ -f "$O/after/$n.png" ] && ! cmp -s "$O/before/$n.png" "$O/after/$n.png"; then
    # ImageMagick refuses images past its policy's limits (a large map): the pair stays in before/
    # and after/, without a sheet
    { convert "$O/before/$n.png" -gravity north -background white -splice 0x40 -pointsize 22 -annotate +0+8 before -bordercolor '#bbb' -border 2 "$O/a.png"
    convert "$O/after/$n.png" -gravity north -background white -splice 0x40 -pointsize 22 -annotate +0+8 after -bordercolor '#bbb' -border 2 "$O/b.png"
    convert "$O/a.png" "$O/b.png" -gravity north -background white +append -resize '1400x>' \
      -gravity north -splice 0x36 -pointsize 20 -annotate +0+6 "$n" "$O/sheets/$n.png"; } 2>/dev/null ||
      echo "no sheet (too large): $n"
    echo "changed: $n"
  fi
done
rm -f "$O/a.png" "$O/b.png"
echo "sheets: $O/sheets ($(ls "$O/sheets" | wc -l))"
