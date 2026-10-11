#!/usr/bin/env bash
# Installs pinned validators into a cached tool dir: .NET SDK via dotnet-install.sh + tools/ooxml-validate (Open XML SDK, Office2019), and LibreOffice headless from apt. Versions in scripts/ci/versions.env
# Stub: exits 0 so CI stays green until the implementing issue fills it in.
# Definition of done for that issue: delete the NOT IMPLEMENTED line below and
# ask Jared (or the Chief of Staff's gh login) to add the job to the
# branch-protection required checks. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
. scripts/ci/versions.env; echo "tool dir: ${NITRO_TOOLS_DIR:-$HOME/.nitro-tools}"
msg="NOT IMPLEMENTED — see #48"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
