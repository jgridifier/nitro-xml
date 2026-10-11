#!/usr/bin/env bash
# IME / OS text input gate: composition events (CJK, dead keys, emoji, dictation) in the Rust editing surface (job: ime)
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."

msg="NOT IMPLEMENTED — see #56"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
