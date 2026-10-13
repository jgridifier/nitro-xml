#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Save-path edit fuzz (#89): short seeded sequences + negative control.
# Not a long campaign (no 1 CPU-hour here). Agents never edit .github/workflows/;
# ask Jared to wire this job into required checks when ready.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

FUZZ_DIR="$ROOT/rs/fuzz"

echo "## edit_fuzz (#89)"
echo "CI-sized save-path check: undo-to-start, save/reopen, negative control"

cd "$FUZZ_DIR"
# Prefer the workspace stable toolchain so we do not need libFuzzer/C++ in CI.
# Overnight campaigns: `cargo fuzz run edit_xlsx --features libfuzzer …`.
cargo +stable run -q --bin edit_ci --release --offline 2>/dev/null \
  || cargo +stable run -q --bin edit_ci --release

msg="edit_fuzz: xlsx/docx/pptx seeded sequences + negative controls passed"
echo "$msg"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  echo "$msg" >>"$GITHUB_STEP_SUMMARY"
fi
