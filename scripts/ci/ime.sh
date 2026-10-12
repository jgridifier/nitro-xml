#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# IME / CJK composition gate (#56): CDP Input.imeSetComposition + insertText
# parity vs textarea oracle; composition caret within 1 CSS px (#43).
#
# Definition of done: ask Jared (or the Chief of Staff's gh login) to add the
# `ime` job to branch-protection required checks. Agents never edit .github/workflows/.
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

echo "ime: rust unit tests (text::ime)"
( cd "$ROOT/rs" && cargo test -p nitro-xml-wasm --lib text::ime:: -- --nocapture )

echo "ime: build wasm"
bash scripts/build-wasm.sh

echo "ime: installing Playwright (chromium + webkit + firefox nightly smoke)"
( cd parity/visual && bun install )
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )
( cd parity/visual && bunx playwright install webkit firefox ) || true

echo "ime: CJK composition parity vs textarea"
( cd "$ROOT" && bun tests/ime/ime_parity.mjs )

echo "ime: PASS"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/ime/out/gate-summary.txt ]; then
  {
    echo "## IME gate (#56)"
    echo '```'
    cat tests/ime/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 0
