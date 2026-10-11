#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# All arms on one issue from one base commit, in rotated start order, in parallel worktrees.
# bash tools/model-compare/run-issue.sh <issue> [base-sha] [arms...]
set -uo pipefail
N=$1; BASE=${2:-$(git rev-parse origin/main)}; shift 2 || shift $#
ARMS=(${@:-A-opus B-opus-haiku C-codex D-opencode})
k=$(( N % ${#ARMS[@]} )); ORDER=("${ARMS[@]:k}" "${ARMS[@]:0:k}")
echo "issue #$N base $BASE order: ${ORDER[*]}"
for a in "${ORDER[@]}"; do bash "$(dirname "$0")/run.sh" "$a" "$N" "$BASE" & sleep 20; done; wait
