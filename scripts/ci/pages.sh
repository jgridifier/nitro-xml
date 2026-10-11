#!/usr/bin/env bash
# Assembles the Pages site into _site/: the plan (docs/) at /, the perf
# scoreboard at /perf/ (filled in by the dashboard issue). Called by pages.yml.
set -euo pipefail
cd "$(dirname "$0")/../.."
rm -rf _site && mkdir -p _site && cp -R docs/. _site/
msg="NOT IMPLEMENTED — see #9 (perf scoreboard); plan pages copied only"; echo "$msg"; if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then echo "$msg" >> "$GITHUB_STEP_SUMMARY"; fi
exit 0
