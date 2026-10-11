#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #11 (fixed before the first comparison run). Differential vs upstream readZipParts (JSZip).
set -uo pipefail
cd "$(dirname "$0")/../../.."
ROOT=$PWD; export PATH=$HOME/.cargo/bin:$PATH
UPSTREAM_DIR=${UPSTREAM_DIR:-/workspace/ooxml-src}; export UPSTREAM_DIR   # same commit e66aaec, with node_modules installed
fail=0; note(){ echo "GATE: $*"; }
[ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" = e66aaec24adc0b68472eae1fc42aa4598e09c352 ] || { note "upstream not at pin"; exit 1; }
( cd rs && cargo fmt --all --check ) || { note "FAIL fmt"; fail=1; }
( cd rs && cargo clippy --workspace --all-targets -q -- -D warnings ) || { note "FAIL clippy"; fail=1; }
( cd rs && cargo test --workspace -q 2>&1 | tail -3 ) || { note "FAIL cargo test"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-opc --target wasm32-unknown-unknown ) || { note "FAIL wasm32 build"; fail=1; }
if ( cd rs && cargo tree -p nitro-xml-opc -e normal --prefix none ) | grep -Eiq '^(bzip2|zstd|lzma|xz2|zstd-sys|bzip2-sys|lzma-sys|libz-sys|zopfli|deflate64|aes|pbkdf2)'; then note "FAIL forbidden zip features/C deps"; fail=1; fi
( cd rs && cargo build --release -q -p nitro-xml-opc --example zip_dump ) || { note "FAIL zip_dump does not build"; exit 1; }
BIN=rs/target/release/examples/zip_dump
TMP=$(mktemp -d); python3 tools/model-compare/gates/make_malformed.py "$TMP/mal" fixtures/real/xlsx/usgov-census-NST-EST2024-POP.xlsx
n=0; bad=0
for f in fixtures/real/*/*.{xlsx,xlsm,xltx,docx,docm,dotx,pptx,pptm,potx} "$TMP"/mal/*; do
  [ -f "$f" ] || continue; n=$((n+1))
  ref=$(bun tools/model-compare/gates/ref/zip_ref.ts "$f" 2>/dev/null); rc_ref=$?
  got=$(timeout 60 "$BIN" "$f" 2>/dev/null); rc=$?
  if [ $rc -eq 101 ] || [ $rc -eq 124 ] || [ $rc -gt 128 ]; then note "FAIL panic/timeout/crash rc=$rc on $f"; bad=$((bad+1)); continue; fi
  if [ $rc_ref -ne 0 ] || [ $rc -ne 0 ]; then
    if [ $rc_ref -ne 0 ] && [ $rc -ne 0 ]; then continue; fi
    note "FAIL accept/reject mismatch on $f (ref rc=$rc_ref, rust rc=$rc)"; bad=$((bad+1)); continue
  fi
  if [ "$(echo "$ref" | python3 -c 'import json,sys;print(json.dumps(json.load(sys.stdin),sort_keys=True))')" != "$(echo "$got" | python3 -c 'import json,sys;print(json.dumps(json.load(sys.stdin),sort_keys=True))' 2>/dev/null)" ]; then
    note "FAIL output differs on $f"; bad=$((bad+1)); fi
done
rm -rf "$TMP"
note "differential: $((n-bad))/$n files match upstream"
[ $bad -eq 0 ] || fail=1
[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
