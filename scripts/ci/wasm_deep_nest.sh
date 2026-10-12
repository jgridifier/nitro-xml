#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Browser WASM open-and-render for MAX_XML_DEPTH fixtures (#30 / #178).
# Confirms a 4096-deep worksheet shell opens+paints (or over-max rejects)
# in Chromium without a WASM stack trap.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

echo "## wasm-deep-nest"
bun tests/fuzz/wasm-deep-nest.mjs
msg="wasm-deep-nest: browser open-and-render completed without trap"
echo "$msg"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  echo "$msg" >>"$GITHUB_STEP_SUMMARY"
fi
