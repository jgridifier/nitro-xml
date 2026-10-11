#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #20: differential vs upstream styles/numbering/theme/settings (no speed testing).
set -uo pipefail
cd "$(dirname "$0")/../../.."
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
UPSTREAM_DIR=${UPSTREAM_DIR:-/workspace/ooxml-src}; export UPSTREAM_DIR
fail=0; note(){ echo "GATE: $*"; }
[ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" = e66aaec24adc0b68472eae1fc42aa4598e09c352 ] || { note "upstream not at pin"; exit 1; }
( cd rs && cargo fmt --all --check ) || { note "FAIL fmt"; fail=1; }
( cd rs && cargo clippy --workspace --all-targets -q -- -D warnings ) || { note "FAIL clippy"; fail=1; }
( cd rs && cargo test --workspace -q 2>&1 | tail -5 ) || { note "FAIL cargo test"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-opc --target wasm32-unknown-unknown ) || { note "FAIL wasm32 opc"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-wasm --target wasm32-unknown-unknown ) || { note "FAIL wasm32 wasm"; fail=1; }
python3 tools/model-compare/gates/make_opc_fixtures.py >/dev/null
( cd rs && cargo build --release -q -p nitro-xml-docx --example docx_styles_dump ) || { note "FAIL docx_styles_dump does not build"; exit 1; }
BIN=rs/target/release/examples/docx_styles_dump
canon(){ python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))'; }

n=0; bad=0
for f in fixtures/real/*/*.{docx,docm,dotx,dotm} fixtures/opc/allowed/minimal.{docx,docm,dotx,dotm}; do
  [ -f "$f" ] || continue
  n=$((n+1))
  ref=$(bun tools/model-compare/gates/ref/docx_styles_ref.ts "$f" 2>/dev/null); rc_ref=$?
  got=$(timeout 60 "$BIN" "$f" 2>/dev/null); rc=$?
  if [ $rc -eq 101 ] || [ $rc -eq 124 ] || [ $rc -gt 128 ]; then note "FAIL panic/timeout/crash rc=$rc on $f"; bad=$((bad+1)); continue; fi
  if [ $rc_ref -ne 0 ] || [ $rc -ne 0 ]; then
    if [ $rc_ref -ne 0 ] && [ $rc -ne 0 ]; then continue; fi
    note "FAIL accept/reject mismatch on $f (ref rc=$rc_ref, rust rc=$rc)"; bad=$((bad+1)); continue
  fi
  if [ "$(echo "$ref" | canon)" != "$(echo "$got" | canon 2>/dev/null)" ]; then
    note "FAIL output differs on $f"; bad=$((bad+1))
  fi
done
note "styles differential: $((n-bad))/$n files match upstream"
[ $bad -eq 0 ] || fail=1

[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
