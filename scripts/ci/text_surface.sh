#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Single-line text surface gate (#55): unit tests + caret/selection parity vs
# textarea oracle within 1 CSS px (#43 tolerance).
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

rustup toolchain install --no-self-update >/dev/null

echo "text_surface: rust unit tests"
( cd "$ROOT/rs" && cargo test -p nitro-xml-wasm --lib text:: -- --nocapture )

echo "text_surface: build wasm"
bash scripts/build-wasm.sh

echo "text_surface: caret/selection parity vs textarea"
( cd "$ROOT" && bun tests/text/caret_parity.mjs )

echo "text_surface: PASS"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/text/out/gate-summary.txt ]; then
  {
    echo "## Text surface gate (#55)"
    echo '```'
    cat tests/text/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 0
