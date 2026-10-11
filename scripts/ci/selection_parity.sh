#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Selection and caret parity (#43): scripted selections copy the same text as
# the TS reference; caret/highlight rectangles within 1 CSS px (job: selection-parity).
#
# Definition of done for #43: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `selection-parity`
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

# Pinned upstream submodule (TS oracle).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "selection: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "selection: ensuring MIT Arabic/Hebrew fixtures (shared with #44)"
python3 fixtures/bidi/generate.py

echo "selection: building wasm (shell select/copy + live viewers)"
bash scripts/build-wasm.sh

echo "selection: installing visual host deps (Playwright from #45)"
( cd parity/visual && bun install )
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )

echo "selection: preparing upstream viewer bundles (xlsx-editor + pptx-slide)"
( cd parity/visual && bun prepare.mjs )

echo "selection: running shell + live checks"
set +e
( cd tests/selection && bun ./run.mjs )
rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/selection/out/gate-summary.txt ]; then
  {
    echo "## Selection gate (#43)"
    echo '```'
    cat tests/selection/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "selection: FAIL" >&2
  exit 1
fi
echo "selection: PASS"
echo "Please add the \`selection-parity\` job to branch-protection required checks (Jared / Chief of Staff)."
exit 0
