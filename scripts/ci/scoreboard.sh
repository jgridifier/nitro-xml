#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Public scoreboard races (job: scoreboard). Builds pinned competitor adapters, runs the parse
# race under the documented fairness rules, gates structural invariants, and publishes
# docs/scoreboard/data/parse-latest.json for the Pages dashboard.
#
# Definition of done for #39: this script no longer prints NOT IMPLEMENTED. Ask Jared (or the
# Chief of Staff's gh login) to add the `scoreboard` job to branch-protection required checks,
# and to give the job `contents: write` (plus SCOREBOARD_PUSH=1) if main should commit fresh
# results automatically — agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
# shellcheck disable=SC1091
source scripts/ci/versions.env

export PATH="${HOME}/.bun/bin:${HOME}/.cargo/bin:${PATH}"
export CHROME_PATH="${CHROME_PATH:-${SCOREBOARD_CHROME_PATH:-/usr/bin/google-chrome}}"

summary() {
  local msg="$1"
  echo "$msg"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
    echo "$msg" >> "$GITHUB_STEP_SUMMARY"
  fi
}

if [ ! -x "$CHROME_PATH" ] && [ -x /usr/bin/google-chrome-stable ]; then
  CHROME_PATH=/usr/bin/google-chrome-stable
fi
if [ ! -x "$CHROME_PATH" ]; then
  echo "scoreboard: Chrome not found at CHROME_PATH=$CHROME_PATH" >&2
  exit 1
fi

# Upstream TS reference (pinned submodule).
if [ ! -f upstream/package.json ]; then
  git submodule update --init upstream
fi
if [ ! -d upstream/src/core/dist ]; then
  summary "scoreboard: building upstream ooxml-core @ $(git -C upstream rev-parse --short HEAD)"
  if ! command -v bun >/dev/null; then
    curl -fsSL https://bun.sh/install | bash
    export PATH="${HOME}/.bun/bin:${PATH}"
  fi
  (cd upstream && bun install --frozen-lockfile && bun run build)
fi

cd perf/scoreboard
summary "scoreboard: npm ci (pinned competitors)"
npm ci --ignore-scripts

summary "scoreboard: selftest (pins, equivalence ranking, Excel serial edge)"
node races/parse/selftest.mjs

summary "scoreboard: build Rust competitor shims (wasm32 release + wasm-opt)"
bash adapters/rust/build.sh

summary "scoreboard: bundle JS adapters"
node build.mjs

SEED="${SCOREBOARD_SEED:-$SCOREBOARD_SEED_DEFAULT}"
WARMUP="${SCOREBOARD_WARMUP:-$SCOREBOARD_WARMUP_DEFAULT}"
RUNS="${SCOREBOARD_RUNS:-$SCOREBOARD_RUNS_DEFAULT}"
# Optional quick path for local debugging only — full corpus is the gate.
EXTRA_ARGS=()
if [ -n "${SCOREBOARD_FILES:-}" ]; then EXTRA_ARGS+=(--files="$SCOREBOARD_FILES"); fi
if [ -n "${SCOREBOARD_FORMAT:-}" ]; then EXTRA_ARGS+=(--format="$SCOREBOARD_FORMAT"); fi
if [ -n "${SCOREBOARD_ENTRANTS:-}" ]; then EXTRA_ARGS+=(--entrants="$SCOREBOARD_ENTRANTS"); fi

OUT="results/parse/$(date -u +%Y-%m-%d)-$(git -C ../.. rev-parse --short HEAD).json"
summary "scoreboard: parse race (warmup=$WARMUP timed=$RUNS seed=$SEED chrome=$CHROME_PATH)"
node run.mjs --warmup="$WARMUP" --runs="$RUNS" --seed="$SEED" --chrome="$CHROME_PATH" --out="$OUT" "${EXTRA_ARGS[@]}"

summary "scoreboard: gate"
# Filtered local debug may lower run counts; the publishable path keeps the defaults.json floor.
if [ -n "${SCOREBOARD_FILES:-}" ] || [ -n "${SCOREBOARD_FORMAT:-}" ]; then
  export SCOREBOARD_MIN_RUNS="${SCOREBOARD_MIN_RUNS:-1}"
  export SCOREBOARD_MIN_WARMUP="${SCOREBOARD_MIN_WARMUP:-0}"
fi
node races/parse/gate.mjs "$OUT"

# Publish into docs/ for Pages (pages.sh copies docs/ → _site/).
mkdir -p ../../docs/scoreboard/data
cp "$OUT" ../../docs/scoreboard/data/parse-latest.json

# Step summary: wins and losses equally.
{
  echo "## Scoreboard parse race"
  echo ""
  echo "- Result: \`perf/scoreboard/${OUT}\`"
  echo "- Published: \`docs/scoreboard/data/parse-latest.json\`"
  echo "- Method: \`docs/scoreboard-method.md\`"
  echo "- Runs: ${WARMUP} warm-up + ${RUNS} timed, seed ${SEED}"
  echo ""
  echo "### Per-format summary (losses listed with wins)"
  echo ""
  node --input-type=module -e "
import { readFileSync } from 'node:fs';
const r = JSON.parse(readFileSync(process.argv[1], 'utf8'));
const ref = r.method.reference;
for (const [fmt, rows] of Object.entries(r.summary)) {
  console.log('#### ' + fmt);
  console.log('');
  console.log('| Entrant | Status | Median ms | Geomean vs TS | Wins | Losses | Comparable |');
  console.log('|---|---|---:|---:|---:|---:|---:|');
  const ordered = Object.entries(rows).sort((a, b) => (a[1].medianOfMediansMs ?? 1e9) - (b[1].medianOfMediansMs ?? 1e9));
  for (const [id, row] of ordered) {
    const name = (r.entrants.find((e) => e.id === id) || {}).name || id;
    if (row.status !== 'entered') {
      console.log('| ' + name + ' | ' + row.status + ' | | | | | |');
      continue;
    }
    const ratio = id === ref ? '1' : (row.geomeanRatioVsReference == null ? 'n/a' : row.geomeanRatioVsReference.toFixed(3));
    const med = row.medianOfMediansMs == null ? '' : row.medianOfMediansMs.toFixed(2);
    console.log('| ' + name + (id === ref ? ' (reference)' : '') + ' | entered | ' + med + ' | ' + ratio + ' | ' + (row.winsVsReference ?? '') + ' | ' + (row.lossesVsReference ?? '') + ' | ' + row.comparable + '/' + row.files + ' |');
  }
  console.log('');
}
" "$OUT"
} | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"

# Optional commit-back on main when the workflow grants contents:write and sets SCOREBOARD_PUSH=1.
if [ "${SCOREBOARD_PUSH:-}" = "1" ] && [ "${GITHUB_REF:-}" = "refs/heads/main" ]; then
  cd ../..
  git add docs/scoreboard/data/parse-latest.json "perf/scoreboard/${OUT}"
  if git diff --cached --quiet; then
    summary "scoreboard: no result delta to push"
  else
    git -c user.name='nitro-xml-scoreboard' -c user.email='scoreboard@users.noreply.github.com' \
      commit -m "scoreboard: publish parse race $(git rev-parse --short HEAD)"
    git push origin HEAD:main
    summary "scoreboard: pushed parse-latest.json to main for Pages"
  fi
fi

summary "scoreboard: done"
