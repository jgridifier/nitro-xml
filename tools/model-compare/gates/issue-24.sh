#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #24: differential vs upstream PptxHandler load → issue-24 dump JSON (no speed testing).
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
( cd rs && cargo build --release -q -p nitro-xml-pptx --example pptx_dump ) || { note "FAIL pptx_dump build"; exit 1; }

if ! bun parity/run --ci --format pptx; then
  note "FAIL pptx parity (unexpected diffs vs upstream)"
  fail=1
fi

[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
