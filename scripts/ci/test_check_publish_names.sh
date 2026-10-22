#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Catches: check_publish_names.sh must stay read-only (no register/publish) and
# must document the recommended triple (npm nitro-xml, crates nitro-xml, PyPI nitro-xml).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SCRIPT="$ROOT/scripts/ci/check_publish_names.sh"
DOC="$ROOT/docs/PUBLISHING_NAMES.md"

# Catches: script must refuse to imply registration / must say read-only.
grep -q 'Does not register or publish' "$SCRIPT"
grep -q 'Never registers or publishes' "$SCRIPT"

# Catches: recommendations must stay aligned across script + doc + product name.
grep -q 'nitro-xml' "$SCRIPT"
grep -q '@nitro-xml/react' "$SCRIPT"
grep -q 'Recommended publish name' "$DOC"
grep -q '`nitro-xml`' "$DOC"
grep -q '`@nitro-xml/react`' "$DOC"

# Catches: dry help path must not hit the network in a way that fails offline CI
# for --help (no curl).
bash "$SCRIPT" --help | grep -q 'Never registers or publishes'

# Live check when network is available (report-only so a taken name fails the
# follow-up packaging work, not this unit smoke).
if curl -sI -A 'nitro-xml-name-check/0.1' https://registry.npmjs.org/ >/dev/null 2>&1; then
  bash "$SCRIPT" --report-only
else
  echo "test_check_publish_names: skip live registry probe (offline)"
fi

echo "test_check_publish_names: ok"
