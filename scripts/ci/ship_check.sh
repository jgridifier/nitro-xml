#!/usr/bin/env bash
# Ship check: fails if any shipped artefact (packages/*, npm tarball, wasm imports) references the TS engine (ooxml-core, ooxml-ui, upstream/), ProseMirror or Lit (job: ship-check)
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
msg="NOT IMPLEMENTED — see #38"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
