#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #13: differential vs upstream SourceIndex / hyperlinkPolicy (no speed testing).
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
python3 tools/model-compare/gates/make_safe_href_corpus.py >/dev/null
( cd rs && cargo build --release -q -p nitro-xml-opc --example opc_dump ) || { note "FAIL opc_dump does not build"; exit 1; }
BIN=rs/target/release/examples/opc_dump
canon(){ python3 -c 'import json,sys; print(json.dumps(json.load(sys.stdin),sort_keys=True,separators=(",",":")))'; }

# --- package parity over real + generated allowed corpus ---
n=0; bad=0
for f in fixtures/real/*/*.{xlsx,xlsm,xltx,docx,docm,dotx,pptx,pptm,potx} fixtures/opc/allowed/*; do
  [ -f "$f" ] || continue
  n=$((n+1))
  ref=$(bun tools/model-compare/gates/ref/opc_ref.ts package "$f" 2>/dev/null); rc_ref=$?
  got=$(timeout 60 "$BIN" package "$f" 2>/dev/null); rc=$?
  if [ $rc -eq 101 ] || [ $rc -eq 124 ] || [ $rc -gt 128 ]; then note "FAIL panic/timeout/crash rc=$rc on $f"; bad=$((bad+1)); continue; fi
  if [ $rc_ref -ne 0 ] || [ $rc -ne 0 ]; then
    if [ $rc_ref -ne 0 ] && [ $rc -ne 0 ]; then continue; fi
    note "FAIL accept/reject mismatch on $f (ref rc=$rc_ref, rust rc=$rc)"; bad=$((bad+1)); continue
  fi
  if [ "$(echo "$ref" | canon)" != "$(echo "$got" | canon 2>/dev/null)" ]; then
    note "FAIL output differs on $f"; bad=$((bad+1))
  fi
done
note "package differential: $((n-bad))/$n files match upstream"
[ $bad -eq 0 ] || fail=1

# --- negative fixtures must be UnsupportedFormat ---
neg_bad=0; neg_n=0
for f in fixtures/opc/negative/*; do
  [ -f "$f" ] || continue
  neg_n=$((neg_n+1))
  got=$(timeout 30 "$BIN" package "$f" 2>/dev/null); rc=$?
  if [ $rc -eq 0 ]; then note "FAIL negative accepted: $f"; neg_bad=$((neg_bad+1)); continue; fi
  echo "$got" | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d.get("error")=="unsupported_format" and d.get("detected"), d' \
    || { note "FAIL negative shape on $f: $got"; neg_bad=$((neg_bad+1)); }
done
note "negatives: $((neg_n-neg_bad))/$neg_n rejected as unsupported_format"
[ $neg_bad -eq 0 ] || fail=1

# --- safe-href corpus vs upstream hyperlinkPolicy (href via @file so NULs survive) ---
href_bad=0; href_n=0
HREF_TMP=$(mktemp -d)
while IFS= read -r line; do
  [ -n "$line" ] || continue
  href_n=$((href_n+1))
  id=$(python3 -c 'import json,sys; print(json.loads(sys.argv[1])["id"])' "$line")
  arg=$(python3 -c 'import json,sys; from pathlib import Path; o=json.loads(sys.argv[1]); h=o["href"]; p=Path(sys.argv[2])
if h is None: print("__NULL__")
else:
  p.write_bytes(h.encode("utf-8")); print("@"+str(p))' "$line" "$HREF_TMP/h")
  ref=$(bun tools/model-compare/gates/ref/opc_ref.ts href "$arg" 2>/dev/null); rc_ref=$?
  got=$("$BIN" href "$arg" 2>/dev/null); rc=$?
  if [ $rc_ref -ne 0 ] || [ $rc -ne 0 ] || [ "$(echo "$ref" | canon)" != "$(echo "$got" | canon 2>/dev/null)" ]; then
    note "FAIL href mismatch on $id"; href_bad=$((href_bad+1))
  fi
done < fixtures/opc/safe-href.jsonl
rm -rf "$HREF_TMP"
note "safe-href differential: $((href_n-href_bad))/$href_n match upstream"
[ $href_bad -eq 0 ] || fail=1

[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
