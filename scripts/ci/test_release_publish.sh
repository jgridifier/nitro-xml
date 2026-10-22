#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Catches: release driver missing VERSION single-source or defaulting to a real publish.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Catches: accidental real publish as the default.
grep -q 'DRY_RUN="${DRY_RUN:-1}"' scripts/ci/release_publish.sh
grep -q 'DRY_RUN=1' .github/workflows/release.yml || grep -q 'dry_run' .github/workflows/release.yml

# Catches: tag/version drift — VERSION must exist and be semver-ish.
ver="$(tr -d '[:space:]' < VERSION)"
[[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+ ]]

# Catches: workflow not wired to the script / missing OIDC permissions for provenance.
grep -q 'scripts/ci/release_publish.sh' .github/workflows/release.yml
grep -q 'id-token: write' .github/workflows/release.yml

echo "test_release_publish: ok (VERSION=$ver)"
