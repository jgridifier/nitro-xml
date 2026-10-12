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

if command -v bun >/dev/null 2>&1; then
  echo "chrome_gate: negative control (node+bun if DOM available)"
  # Headless check via wasm-only path when no browser.
  (cd "$ROOT/rs" && cargo test -p nitro-xml-wasm ribbon_hidden_in_readonly -- --nocapture) || exit 1
  echo "chrome_gate: PASS (ribbon readonly negative covered in wasm tests)"
else
  echo "chrome_gate: bun missing; wasm tests only"
fi
