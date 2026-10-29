#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# History + regression gate (#8). Called by perf.sh. Usage: perf_history.sh gate|record <out_dir>
#   gate   PR: measure S/M at 1x, compare to the last 5 main entries (perf-data branch), never append.
#   record main/nightly/dispatch: measure (S/M/L, 1x and 4x), append to history.jsonl on perf-data.
# Env: PERF_HISTORY_FIXTURES (default xlsx-S,xlsx-M,docx-S,docx-M,pptx-S,pptx-M), PERF_ITER_COLD,
#      PERF_LABELS / PR_BODY (override, see perf/gate.mjs). Never edits .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode="$1"; OUT="$2"
FIX="${PERF_HISTORY_FIXTURES:-xlsx-S,xlsx-M,docx-S,docx-M,pptx-S,pptx-M}"
HIST="$OUT/history.jsonl"

# History lives on the perf-data branch (main stays clean); fall back to the seeded copy on this branch.
if git fetch --depth=1 origin perf-data 2>/dev/null && git show FETCH_HEAD:perf/history.jsonl >"$HIST" 2>/dev/null; then
  echo "perf_history: history from origin/perf-data ($(wc -l <"$HIST") lines)"
else
  cp perf/history.jsonl "$HIST"
  echo "perf_history: no perf-data branch yet; using committed perf/history.jsonl"
fi

(cd perf && npm ci --silent)
# run.mjs needs perf/dist/ts/*.js (upstream TS bundles); build them once (bun is installed by perf.sh).
export PATH="$HOME/.bun/bin:$PATH"
if [ ! -f perf/dist/ts/xlsx.js ] || [ ! -f perf/dist/ts/docx.js ] || [ ! -f perf/dist/ts/pptx.js ]; then
  git submodule update --init --depth 1 upstream
  node perf/build-ts.mjs
fi
RUNS="$OUT/runs.jsonl"; : >"$RUNS"
throttles=(1); [ "$mode" = record ] && throttles=(1 4)
[ "$mode" = record ] && FIX="${PERF_HISTORY_FIXTURES:-$FIX,xlsx-L,docx-L,pptx-L}"
for t in "${throttles[@]}"; do
  for m in cold warm; do
    node perf/run.mjs --engine ts,wasm --mode "$m" --cpu-throttle "$t" --fixtures "$FIX" \
      --iterations "${PERF_ITER:-$([ "$m" = cold ] && echo 10 || echo 20)}" --out "$OUT/run-$m-$t.json"
    cat "$OUT/run-$m-$t.jsonl" >>"$RUNS"
  done
done

if [ "$mode" = gate ]; then
  node perf/history-append.mjs --from "$RUNS" --history "$OUT/current.jsonl" --branch "${GITHUB_HEAD_REF:-pr}" --ci-job "${GITHUB_RUN_ID:-}"
  node perf/gate.mjs --current "$OUT/current.jsonl" --history "$HIST" --summary "$OUT/gate.md" --override-out "$OUT/override.json" \
    && rc=0 || rc=$?
  [ -n "${GITHUB_STEP_SUMMARY:-}" ] && cat "$OUT/gate.md" >>"$GITHUB_STEP_SUMMARY"
  exit "$rc"
fi

# record: append (plus any override record) and push to perf-data.
node perf/history-append.mjs --from "$RUNS" --history "$HIST" --branch main --ci-job "${GITHUB_RUN_ID:-}"
if [ -n "${PERF_OVERRIDE_REASON:-}" ]; then
  echo "{\"type\":\"override\",\"commit\":\"$(git rev-parse HEAD)\",\"date\":\"$(date -u +%FT%TZ)\",\"reason\":\"$PERF_OVERRIDE_REASON\"}" >"$OUT/override.json"
  node perf/history-append.mjs --from "$OUT/override.json" --history "$HIST"
fi
if [ -n "${GITHUB_ACTIONS:-}" ]; then
  short=$(git rev-parse --short HEAD)
  tmp=$(mktemp -d)
  if git fetch --depth=1 origin perf-data 2>/dev/null; then
    git worktree add -B perf-data "$tmp" FETCH_HEAD >/dev/null
  else
    git worktree add --detach "$tmp" HEAD >/dev/null
    git -C "$tmp" checkout --orphan perf-data >/dev/null
    git -C "$tmp" rm -rfq . 2>/dev/null || true
  fi
  mkdir -p "$tmp/perf" && cp "$HIST" "$tmp/perf/history.jsonl"
  git -C "$tmp" add -f perf/history.jsonl
  git -C "$tmp" -c user.name=perf-bot -c user.email=perf-bot@users.noreply.github.com commit -qm "perf: record $short"
  git -C "$tmp" push origin perf-data
fi
