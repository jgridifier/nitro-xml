#!/usr/bin/env bash
# Hard per-format gate: WASM beats the TS reference at every tier, 1x and 4x, on first paint (incl. wasm fetch/compile/instantiate), cold parse, scroll/view-query latency and peak memory; pass = lower median AND 95% bootstrap CI of WASM/TS ratio entirely below 1.0; a tie fails. Usage: perf_beats_ts.sh <xlsx|docx|pptx> (job: perf-beats-ts)
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
fmt="${1:?format}"; echo "perf_beats_ts.sh format=$fmt"
case "$fmt" in xlsx) n="#19";; docx) n="#23";; pptx) n="#27";; *) echo "unknown format $fmt"; exit 2;; esac
msg="NOT IMPLEMENTED — see $n (gate machinery in #8); format=$fmt"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
