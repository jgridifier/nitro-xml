#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #12 (fixed before the comparison run). Differential vs upstream parseXml (@xmldom/xmldom).
set -uo pipefail
cd "$(dirname "$0")/../../.."
export PATH=$HOME/.cargo/bin:$PATH
UPSTREAM_DIR=${UPSTREAM_DIR:-/workspace/ooxml-src}; export UPSTREAM_DIR
fail=0; note(){ echo "GATE: $*"; }
[ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" = e66aaec24adc0b68472eae1fc42aa4598e09c352 ] || { note "upstream not at pin"; exit 1; }
( cd rs && cargo fmt --all --check ) || { note "FAIL fmt"; fail=1; }
( cd rs && cargo clippy --workspace --all-targets -q -- -D warnings ) || { note "FAIL clippy"; fail=1; }
( cd rs && cargo test --workspace -q 2>&1 | tail -3 ) || { note "FAIL cargo test"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-opc --target wasm32-unknown-unknown ) || { note "FAIL wasm32 build"; fail=1; }
[ -n "$(ls fixtures/xml/*.xml 2>/dev/null)" ] || { note "FAIL no upstream test inputs under fixtures/xml/"; fail=1; }
( cd rs && cargo build --release -q -p nitro-xml-opc --example xml_dump ) || { note "FAIL xml_dump does not build"; exit 1; }
TMP=$(mktemp -d); python3 tools/model-compare/gates/make_xml_cases.py "$TMP" >/dev/null
bun tools/model-compare/gates/ref/xml_ref.ts "$TMP/list.txt" > "$TMP/ref.jsonl"
timeout 300 rs/target/release/examples/xml_dump "$TMP/list.txt" > "$TMP/got.jsonl"; rc=$?
if [ $rc -ne 0 ]; then note "FAIL xml_dump exited $rc (panic=101, timeout=124)"; fail=1; fi
python3 - "$TMP/ref.jsonl" "$TMP/got.jsonl" <<'PY' || fail=1
import json, sys
ref = [json.loads(l) for l in open(sys.argv[1])]; got = {}
for l in open(sys.argv[2]):
    try: o = json.loads(l); got[o['file']] = o
    except Exception: pass
bad = 0
for r in ref:
    g = got.get(r['file'])
    if g is None: print('GATE: FAIL missing output for', r['file']); bad += 1; continue
    if ('error' in r) != ('error' in g):
        print('GATE: FAIL accept/reject mismatch on', r['file'], '(ref', 'error' if 'error' in r else 'ok', ')'); bad += 1; continue
    if 'tree' in r and r['tree'] != g['tree']:
        print('GATE: FAIL tree differs on', r['file']); bad += 1
print(f'GATE: differential: {len(ref) - bad}/{len(ref)} XML inputs match upstream')
sys.exit(1 if bad else 0)
PY
rm -rf "$TMP"
[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
