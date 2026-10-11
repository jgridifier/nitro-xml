#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copy the Head Of Product's working spec into docs/ and show what changed.
# Usage (from anywhere in the repo): bash scripts/sync-spec.sh
# Override the source with SPEC_SRC=/path/to/PRODUCT_SPEC.md. Commit and push yourself (never force-push).
set -euo pipefail
SRC="${SPEC_SRC:-/workspace/ooxml-rs-plan/PRODUCT_SPEC.md}"
cd "$(git rev-parse --show-toplevel)"
[ -f "$SRC" ] || { echo "spec source not found: $SRC" >&2; exit 1; }
cp "$SRC" docs/PRODUCT_SPEC.md
git diff --stat -- docs/PRODUCT_SPEC.md
