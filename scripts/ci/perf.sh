#!/usr/bin/env bash
# Perf track. Usage: perf.sh gate (PR: S/M tiers, 1x, compare only) | perf.sh record (main/nightly/dispatch: full corpus, 1x and 4x, append to perf-data). Every commit posts its numbers (jobs: perf-gate, perf-record)
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode="${1:-gate}"; echo "perf.sh mode=$mode"
msg="NOT IMPLEMENTED — see #8"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
