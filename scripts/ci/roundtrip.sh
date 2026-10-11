#!/usr/bin/env bash
# Lossless save gate (a): open->save canonical package compare, validator error subset, LibreOffice reopen (job: roundtrip)
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
bash scripts/ci/setup-validators.sh
msg="NOT IMPLEMENTED — see #49"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
