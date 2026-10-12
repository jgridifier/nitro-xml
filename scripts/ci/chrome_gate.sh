#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Editor chrome gate (#53): wasm unit tests + negative control (readonly hide).
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD
export PATH="$HOME/.cargo/bin:$PATH"

echo "chrome_gate: rust chrome tests"
(cd "$ROOT/rs" && cargo test -p nitro-xml-wasm --lib chrome)

echo "chrome_gate: build wasm"
bash scripts/build-wasm.sh

echo "chrome_gate: rust negative control (readonly hide)"
(cd "$ROOT/rs" && cargo test -p nitro-xml-wasm ribbon_hidden_in_readonly -- --nocapture)

if command -v bun >/dev/null 2>&1; then
  echo "chrome_gate: browser negative control"
  bun "$ROOT/tests/chrome/browser-negative.mjs"
fi

echo "chrome_gate: PASS"
