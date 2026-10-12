#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Crash recovery: viewer reload-once in a fresh worker (#30); editor op-journal replay (#62).
# Job: crash-recovery. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

echo "crash-recovery: viewer reload-once — skipped (TODO #30; not gated here)"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

# shellcheck source=ensure-upstream-oracle.sh
. scripts/ci/ensure-upstream-oracle.sh "$ROOT"

echo "crash-recovery: loader journal unit tests"
node --test "$ROOT/packages/nitro-xml/loader/test/journal.test.mjs"

echo "crash-recovery: editor journal + TS oracle (crash injection)"
export NITRO_CRASH_SEED="${NITRO_CRASH_SEED:-62}"
bun "$ROOT/tests/crash/run.mjs" --ci

SUMMARY="crash-recovery: editor op-journal replay OK (seed=${NITRO_CRASH_SEED})"
echo "$SUMMARY"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "## crash-recovery (#62 editor journal)"
    echo "$SUMMARY"
    echo ""
    echo "Viewer reload-once: TODO #30 (not required for this gate)."
    echo "Writer carry / dirty-parts replay: TODO #69."
  } >> "$GITHUB_STEP_SUMMARY"
fi

echo "crash-recovery: PASS"
