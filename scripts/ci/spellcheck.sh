#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Spellcheck gate: browser/OS spellcheck integration on the Rust editing surface (job: spellcheck)
#
# Definition of done for #58: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `spellcheck`
# job to the branch-protection required checks. Agents never edit .github/workflows/.
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

echo "spellcheck: rust unit tests (text::spell)"
(
  cd rs
  cargo test -p nitro-xml-wasm --lib text::spell --locked
)

echo "spellcheck: building wasm"
bash scripts/build-wasm.sh

echo "spellcheck: installing visual host deps (Playwright)"
( cd parity/visual && bun install )
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )

echo "spellcheck: running harness"
set +e
( cd tests/spellcheck && bun ./run.mjs )
rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/spellcheck/out/gate-summary.txt ]; then
  {
    echo "## Spellcheck gate (#58)"
    echo '```'
    cat tests/spellcheck/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "spellcheck: FAIL" >&2
  exit 1
fi
echo "spellcheck: PASS"
echo "Please add the \`spellcheck\` job to branch-protection required checks (Jared / Chief of Staff)."
exit 0
