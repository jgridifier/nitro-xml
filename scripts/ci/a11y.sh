#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Accessibility gate (#42): differential axe-core (zero new serious/critical vs
# upstream TS), CDP Accessibility.getFullAXTree role/name diff, and keyboard
# reachability of Stage-6 shell controls (job: a11y).
# xlsx + pptx now; docx when #158 lands.
#
# Definition of done for #42: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `a11y` job to the
# branch-protection required checks. Agents never edit .github/workflows/.
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

# Pinned upstream submodule (TS oracle hosts).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "a11y: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "a11y: building wasm (shell + viewers)"
bash scripts/build-wasm.sh

echo "a11y: installing visual host deps (Playwright from #45)"
( cd parity/visual && bun install )
( cd parity/visual && bunx playwright install --with-deps chromium ) \
  || ( cd parity/visual && bunx playwright install chromium )

echo "a11y: installing axe-core"
( cd tests/a11y && bun install )

echo "a11y: preparing upstream viewer bundles + font mirror"
( cd parity/visual && bun ./prepare.mjs )

echo "a11y: running differential axe + AX-tree + keyboard"
set +e
( cd tests/a11y && bun ./run.mjs )
rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f tests/a11y/out/gate-summary.txt ]; then
  {
    echo "## Accessibility gate (#42)"
    echo '```'
    cat tests/a11y/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "a11y: FAIL" >&2
  exit 1
fi
echo "a11y: PASS"
exit 0
