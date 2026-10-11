#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Lossless save gate (a): open→save canonical package compare, validator error
# subset, LibreOffice reopen (job: roundtrip). Issue #49.
#
# Definition of done for #49: this script no longer prints NOT IMPLEMENTED.
# Ask Jared (or the Chief of Staff's gh login) to add the `roundtrip` job to
# the branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"
export NITRO_TOOLS_DIR="${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

# Validators (#48): soft-skip inside the harness when tools are absent.
bash scripts/ci/setup-validators.sh || true

# Pinned upstream submodule (TS oracle).
git submodule update --init --depth 1 upstream
export UPSTREAM_DIR="$ROOT/upstream"
if [ "$(git -C "$UPSTREAM_DIR" rev-parse HEAD)" != e66aaec24adc0b68472eae1fc42aa4598e09c352 ]; then
  echo "roundtrip: upstream not at pin e66aaec" >&2
  exit 1
fi
if [ ! -d "$UPSTREAM_DIR/src/core/node_modules" ] && [ ! -d "$UPSTREAM_DIR/node_modules" ]; then
  ( cd "$UPSTREAM_DIR" && bun install )
fi

echo "roundtrip: installing harness deps"
( cd parity/roundtrip && bun install )

echo "roundtrip: TS-vs-TS no-op save over fixtures/manifest.json"
set +e
( cd parity/roundtrip && bun ./run.mjs --ci )
rc=$?
set -e

if [ -n "${GITHUB_STEP_SUMMARY:-}" ] && [ -f parity/roundtrip/out/gate-summary.txt ]; then
  {
    echo "## Round-trip gate (#49)"
    echo '```'
    cat parity/roundtrip/out/gate-summary.txt
    echo '```'
  } >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$rc" -ne 0 ]; then
  echo "roundtrip: FAIL" >&2
  exit 1
fi
echo "roundtrip: PASS"
echo "Please add the \`roundtrip\` job to branch-protection required checks (Jared / Chief of Staff)."
exit 0
