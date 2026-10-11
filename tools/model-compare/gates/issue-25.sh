#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #25: DrawingML colour transforms, fills, themes and preset geometry (no perf).
set -uo pipefail
cd "$(dirname "$0")/../../.."
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
UPSTREAM_DIR=${UPSTREAM_DIR:-/workspace/ooxml-src}; export UPSTREAM_DIR
fail=0; note(){ echo "GATE: $*"; }
HREF_TMP=$(mktemp -d)
trap 'rm -rf "$HREF_TMP"' EXIT
[ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" = e66aaec24adc0b68472eae1fc42aa4598e09c352 ] || { note "upstream not at pin"; exit 1; }
( cd rs && cargo fmt --all --check ) || { note "FAIL fmt"; fail=1; }
( cd rs && cargo clippy -p nitro-xml-opc --all-targets -q -- -D warnings ) || { note "FAIL clippy"; fail=1; }
( cd rs && cargo test -p nitro-xml-opc -q ) || { note "FAIL cargo test"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-opc --target wasm32-unknown-unknown ) || { note "FAIL wasm32 opc"; fail=1; }
( cd rs && cargo build --release -q -p nitro-xml-opc --example drawingml_dump ) || { note "FAIL drawingml_dump build"; exit 1; }
BIN=rs/target/release/examples/drawingml_dump
REF=tools/model-compare/gates/ref/drawingml_ref.ts
canon(){ python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))'; }

# --- colour transform corpus ---
color_n=0; color_bad=0
while IFS= read -r line; do
  [ -n "$line" ] || continue
  color_n=$((color_n+1))
  id=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "$line")
  ref=$(bun "$REF" color "$line" 2>/dev/null); rc_ref=$?
  got=$("$BIN" color "$line" 2>/dev/null); rc=$?
  if [ $rc_ref -ne 0 ] || [ $rc -ne 0 ]; then
    note "FAIL color rc on $id (ref=$rc_ref rust=$rc)"; color_bad=$((color_bad+1)); continue
  fi
  printf '%s' "$ref" > "$HREF_TMP/ref.json"
  printf '%s' "$got" > "$HREF_TMP/got.json"
  python3 tools/model-compare/gates/drawingml_compare.py color "@$HREF_TMP/ref.json" "@$HREF_TMP/got.json" \
    || { note "FAIL color mismatch on $id"; color_bad=$((color_bad+1)); }
done < <(python3 tools/model-compare/gates/drawingml_color_corpus.py)
note "colour transforms: $((color_n-color_bad))/$color_n match upstream"
[ $color_bad -eq 0 ] || fail=1

# --- preset geometry (all presets × sizes × adj variants) ---
preset_n=0; preset_bad=0
SIZES="200 100 100 200 216 216"
names=$(python3 -c 'import json; print(" ".join(json.load(open("rs/crates/nitro-xml-opc/src/drawingml/data/preset_shape_geometry.json")).keys()))')
for name in $names; do
  for pair in "200 100" "100 200" "216 216"; do
    set -- $pair
    w=$1; h=$2
    for variant in default adj; do
      preset_n=$((preset_n+1))
      adj_arg=""
      if [ "$variant" = adj ]; then
        adj_arg=$(python3 -c "
import json,sys
d=json.load(open('rs/crates/nitro-xml-opc/src/drawingml/data/preset_shape_geometry.json'))
av=d.get(sys.argv[1],{}).get('avLst') or {}
if not av: print(''); sys.exit(0)
out={}
for k,v in av.items():
  if isinstance(v,(int,float)):
    out[k]=int(round(v*0.9)) if v else v
print(json.dumps(out,separators=(',',':')))
" "$name")
        [ -z "$adj_arg" ] && continue
      fi
      id="${name}/${w}x${h}/${variant}"
      if [ -n "$adj_arg" ]; then
        ref=$(bun "$REF" preset "$name" "$w" "$h" "$adj_arg" 2>/dev/null)
        got=$("$BIN" preset "$name" "$w" "$h" "$adj_arg" 2>/dev/null)
      else
        ref=$(bun "$REF" preset "$name" "$w" "$h" 2>/dev/null)
        got=$("$BIN" preset "$name" "$w" "$h" 2>/dev/null)
      fi
      printf '%s' "$ref" > "$HREF_TMP/ref.json"
      printf '%s' "$got" > "$HREF_TMP/got.json"
      python3 tools/model-compare/gates/drawingml_compare.py preset "@$HREF_TMP/ref.json" "@$HREF_TMP/got.json" \
        || { note "FAIL preset $id"; preset_bad=$((preset_bad+1)); }
    done
  done
done
note "preset geometry: $((preset_n-preset_bad))/$preset_n match upstream (0.01px paths)"
[ $preset_bad -eq 0 ] || fail=1

# --- theme colour resolution from real theme parts ---
THEME_TMP=$(mktemp -d)
python3 tools/model-compare/gates/extract_drawingml_themes.py . "$THEME_TMP" >/dev/null 2>/dev/null
theme_n=0; theme_bad=0
for f in "$THEME_TMP"/*; do
  [ -f "$f" ] || continue
  [ -f "$f" ] || continue
  theme_n=$((theme_n+1))
  ref=$(bun "$REF" theme-colors "$f" 2>/dev/null)
  got=$("$BIN" theme-colors "$f" 2>/dev/null)
  printf '%s' "$ref" > "$HREF_TMP/ref.json"
  printf '%s' "$got" > "$HREF_TMP/got.json"
  python3 tools/model-compare/gates/drawingml_compare.py theme "@$HREF_TMP/ref.json" "@$HREF_TMP/got.json" \
    || { note "FAIL theme-colors on $f"; theme_bad=$((theme_bad+1)); }
done
rm -rf "$THEME_TMP"
note "theme colours: $((theme_n-theme_bad))/$theme_n theme files match upstream"
[ $theme_bad -eq 0 ] || fail=1

# --- fill/line parse snippets ---
fill_n=0; fill_bad=0
FILL_DIR=tools/model-compare/gates/fixtures/drawingml-fill
mkdir -p "$FILL_DIR"
python3 - <<'PY' "$FILL_DIR"
import pathlib, sys
d = pathlib.Path(sys.argv[1])
d.mkdir(parents=True, exist_ok=True)
snippets = {
    "solid.xml": '<a:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:spPr>',
    "gradient.xml": '<a:ln xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" w="12700"><a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="100000"><a:schemeClr val="accent2"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="1"/></a:gradFill></a:ln>',
    "pattern.xml": '<a:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:pattFill prst="pct5"><a:fgClr><a:srgbClr val="000000"/></a:fgClr><a:bgClr><a:srgbClr val="FFFFFF"/></a:bgClr></a:pattFill></a:spPr>',
    "none.xml": '<a:spPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:noFill/></a:spPr>',
}
for name, xml in snippets.items():
    (d / name).write_text(xml)
PY
for f in "$FILL_DIR"/*.xml; do
  [ -f "$f" ] || continue
  fill_n=$((fill_n+1))
  ref=$(bun "$REF" fill "$f" 2>/dev/null)
  got=$("$BIN" fill "$f" 2>/dev/null)
  printf '%s' "$ref" > "$HREF_TMP/ref.json"
  printf '%s' "$got" > "$HREF_TMP/got.json"
  python3 tools/model-compare/gates/drawingml_compare.py fill "@$HREF_TMP/ref.json" "@$HREF_TMP/got.json" \
    || { note "FAIL fill on $(basename "$f")"; fill_bad=$((fill_bad+1)); }
done
note "fill/line parse: $((fill_n-fill_bad))/$fill_n snippets match upstream"
[ $fill_bad -eq 0 ] || fail=1

[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
