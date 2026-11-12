#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Crash recovery: viewer reload-once in a fresh worker (#30); editor op-journal replay (#62);
# Rust worker killed mid-edit, journal replay + writer carry of untouched parts (#30).
# Job: crash-recovery. Agents never edit .github/workflows/.
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

# shellcheck source=ensure-upstream-oracle.sh
. scripts/ci/ensure-upstream-oracle.sh "$ROOT"

echo "crash-recovery: loader viewer recover unit tests"
node --test "$ROOT/packages/nitro-xml/loader/test/viewer-recover.test.mjs"

echo "crash-recovery: viewer reload harness (#30)"
VIEWER_SEED="${NITRO_VIEWER_CRASH_SEED:-30}"
NITRO_CRASH_SEED="$VIEWER_SEED" bun "$ROOT/tests/crash/viewer_reload.mjs" --ci
VIEWER_SUMMARY="crash-recovery: viewer reload-once OK (seed=${VIEWER_SEED})"
echo "$VIEWER_SUMMARY"

echo "crash-recovery: loader journal + edit-recover unit tests"
node --test \
  "$ROOT/packages/nitro-xml/loader/test/journal.test.mjs" \
  "$ROOT/packages/nitro-xml/loader/test/edit-recover.test.mjs" \
  "$ROOT/packages/nitro-xml/loader/test/edit-recover-defer.test.mjs" \
  "$ROOT/packages/nitro-xml/loader/test/defer-journal.test.mjs" \
  "$ROOT/packages/nitro-xml/loader/test/protections.test.mjs"

echo "crash-recovery: pptx shape drag restore from journal after simulated crash (#30)"
bun "$ROOT/tests/crash/pptx_drag_restore.test.mjs"

echo "crash-recovery: editor journal + TS oracle (crash injection, xlsx/docx/pptx)"
EDITOR_SEED="${NITRO_EDITOR_CRASH_SEED:-62}"
# Default 10 deterministic flake iterations in CI; override with NITRO_CRASH_FLAKE_RUNS.
FLAKE_RUNS="${NITRO_CRASH_FLAKE_RUNS:-10}"
NITRO_CRASH_SEED="$EDITOR_SEED" NITRO_CRASH_FLAKE_RUNS="$FLAKE_RUNS" \
  bun "$ROOT/tests/crash/run.mjs" --ci

EDITOR_SUMMARY="crash-recovery: editor op-journal replay OK (seed=${EDITOR_SEED} formats=3 flake_runs=${FLAKE_RUNS})"
echo "$EDITOR_SUMMARY"
echo "crash-recovery: Rust worker SIGABRT mid-edit, journal replay, writer carry (xlsx/docx/pptx)"
(cd "$ROOT/rs" && cargo test -p nitro-xml --test crash_injection)
RUST_SUMMARY="crash-recovery: Rust worker kill + journal replay + untouched-part carry OK (formats=3)"
echo "$RUST_SUMMARY"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  {
    echo "## crash-recovery"
    echo "$VIEWER_SUMMARY"
    echo "$EDITOR_SUMMARY"
    echo "$RUST_SUMMARY"
    echo ""
    echo "M1 report: docs/M1-REPORT.md §2 (crash recovery ACs)."
  } >> "$GITHUB_STEP_SUMMARY"
fi

echo "crash-recovery: PASS"
