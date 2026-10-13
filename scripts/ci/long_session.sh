#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Long-session correctness gate (#90): 1000-op edit sessions per format.
# Does not measure memory or timing — see TODO(#90) in perf/runner/session.ts.
# Agents never edit .github/workflows/; ask Jared to wire this into required checks.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

if ! command -v bun >/dev/null 2>&1; then
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

summary_file="${TMPDIR:-/tmp}/long-session-summary.txt"
if ( cd "$ROOT/perf/runner" && bun session.ts ) 2>&1 | tee "$summary_file"; then
  line="$(grep '^long-session gate:' "$summary_file" | tail -1 || true)"
  if [[ -z "$line" ]]; then
    line="long-session gate: PASS (no summary line)"
  fi
  echo "$line"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    echo "$line" >>"$GITHUB_STEP_SUMMARY"
  fi
else
  echo "long-session gate: FAIL"
  exit 1
fi
