#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Gate for #12. Differential logic now lives in parity/ (folded by #5); this
# wrapper keeps the model-compare gate surface and still runs fmt/clippy/wasm.
set -uo pipefail
cd "$(dirname "$0")/../../.."
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
ROOT=$PWD
if [ -z "${UPSTREAM_DIR:-}" ]; then
  if [ -d /workspace/ooxml-src/src/core ]; then UPSTREAM_DIR=/workspace/ooxml-src
  else UPSTREAM_DIR=$ROOT/upstream; fi
fi
export UPSTREAM_DIR
fail=0; note(){ echo "GATE: $*"; }
[ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" = e66aaec24adc0b68472eae1fc42aa4598e09c352 ] || { note "upstream not at pin"; exit 1; }
( cd rs && cargo fmt --all --check ) || { note "FAIL fmt"; fail=1; }
( cd rs && cargo clippy --workspace --all-targets -q -- -D warnings ) || { note "FAIL clippy"; fail=1; }
( cd rs && cargo test --workspace -q 2>&1 | tail -3 ) || { note "FAIL cargo test"; fail=1; }
( cd rs && cargo build -q -p nitro-xml-opc --target wasm32-unknown-unknown ) || { note "FAIL wasm32 build"; fail=1; }
[ -n "$(ls fixtures/xml/*.xml 2>/dev/null)" ] || { note "FAIL no upstream test inputs under fixtures/xml/"; fail=1; }
( cd rs && cargo build --release -q -p nitro-xml-opc --example xml_dump ) || { note "FAIL xml_dump does not build"; exit 1; }
if [ ! -d "$UPSTREAM_DIR/node_modules" ]; then ( cd "$UPSTREAM_DIR" && bun install ) || { note "FAIL bun install upstream"; exit 1; }; fi
bun parity/run --format xml --ci || { note "FAIL parity xml"; fail=1; }
[ $fail -eq 0 ] && note "PASS" || note "FAIL"
exit $fail
