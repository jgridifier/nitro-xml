#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Hard per-format gate: WASM beats the TS reference at S/M/L on openDocument time and peak
# memory (VmHWM growth). Pass = lower median AND 95% bootstrap CI of WASM/TS ratio entirely
# below 1.0; a tie fails (PRODUCT_SPEC §5). Usage: perf_beats_ts.sh <xlsx|docx|pptx>
# (job: perf-beats-ts).
#
# Mode (do not lower the bar; do not raise budgets; do not demote fixtures):
#   PERF_GATE_MODE=report   (default) — measure, write the per-test table, exit 0 even on LOSS.
#                            Clearly labeled so Jared can decide when to enforce.
#   PERF_GATE_MODE=enforce  — same table; exit 1 on any LOSS / TIE/LOSS.
#
# Harnesses: perf/large/ts-vs-wasm.mjs (cold openDocument) and perf/large/peak-mem.mjs
# (fresh-process VmHWM). Fixtures generated in-job. XL / 4× CPU / scroll / browser first-paint
# remain TODO(#8) once the Node open+peak baseline is green.
#
# Definition of done for arming: every cell WIN (see enforce-readiness.json / summary footer),
# then Jared flips PERF_GATE_MODE=enforce (or sets it in the workflow env) and asks Jared /
# Chief of Staff to keep `perf-beats-ts` in branch-protection required checks.
# Agents never edit .github/workflows/. The readiness footer never lowers the bar.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

fmt="${1:?format}"
case "$fmt" in
  xlsx) arm_issue="#19";;
  docx) arm_issue="#23";;
  pptx) arm_issue="#27";;
  *) echo "perf_beats_ts.sh: unknown format $fmt (want xlsx|docx|pptx)" >&2; exit 2;;
esac

MODE="${PERF_GATE_MODE:-report}"
case "$MODE" in report|enforce) ;; *)
  echo "perf_beats_ts.sh: PERF_GATE_MODE must be report|enforce (got $MODE)" >&2
  exit 2
esac

TIERS="${PERF_BEATS_TIERS:-S,M,L}"
# OpenDocument samples share one Node process. V8 Liftoff dominates the first ~dozen
# opens; with only 2 warmups the timed set is noisy and ubuntu's 95% CI often spans 1.0
# on xlsx S even when the true ratio is ~0.6. More warmup discards Liftoff; more runs
# tighten the bootstrap CI. Pass rule unchanged (median < 1 AND CI upper < 1).
# Peak-mem knobs untouched (separate fresh-process metric / other agents).
RUNS_OPEN="${PERF_BEATS_OPEN_RUNS:-21}"
WARMUP_OPEN="${PERF_BEATS_OPEN_WARMUP:-8}"
BOOTSTRAP="${PERF_BEATS_BOOTSTRAP:-2000}"
RUNS_MEM="${PERF_BEATS_MEM_RUNS:-7}"
OUT_DIR="${PERF_BEATS_OUT:-/tmp/perf-beats-ts-$(git rev-parse --short HEAD)-$fmt}"
mkdir -p "$OUT_DIR"

echo "perf_beats_ts.sh format=$fmt mode=$MODE tiers=$TIERS arm_when_ready=$arm_issue (gate machinery #8)"
echo "out=$OUT_DIR"

# --- toolchain (thin perf.yml has no setup-* actions; install in-script like other gates) ---
if ! command -v bun >/dev/null 2>&1; then
  echo "perf_beats_ts.sh: installing bun"
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi
if ! command -v rustup >/dev/null 2>&1; then
  echo "perf_beats_ts.sh: rustup missing (needed for wasm build)" >&2
  exit 1
fi
rustup toolchain install --no-self-update >/dev/null

# --- prerequisites: upstream TS oracle + wasm pkg + S/M/L fixtures ---
# shellcheck source=scripts/ci/ensure-upstream-oracle.sh
source "$ROOT/scripts/ci/ensure-upstream-oracle.sh" "$ROOT"

# Build wasm unless caller already built (perf.sh builds once, then sets SKIP=1).
# Matrix jobs leave SKIP unset so each format builds the tip SHA (pkg/ is not committed).
if [ "${PERF_BEATS_SKIP_WASM_BUILD:-0}" = 1 ] \
  && [ -f "$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm" ] \
  && [ -f "$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm.js" ]; then
  echo "perf_beats_ts.sh: reusing existing wasm pkg (PERF_BEATS_SKIP_WASM_BUILD=1)"
else
  echo "perf_beats_ts.sh: building wasm package"
  bash "$ROOT/scripts/build-wasm.sh"
fi

gen_ids=()
case "$fmt" in
  xlsx)
    gen_ids+=(gen-xlsx-cells-1k gen-xlsx-cells-100k gen-xlsx-cells-1m)
    ;;
  docx)
    gen_ids+=(gen-docx-pages-10 gen-docx-pages-100 gen-docx-pages-500)
    ;;
  pptx)
    gen_ids+=(gen-pptx-slides-10 gen-pptx-slides-50 gen-pptx-slides-200)
    ;;
esac
# Only generate recipes whose tier is selected (S/M/L map 1:1 to the three ids above).
selected_ids=()
IFS=',' read -r -a tier_arr <<< "$TIERS"
for id in "${gen_ids[@]}"; do
  case "$id" in
    *-1k|*-pages-10|*-slides-10) want=S ;;
    *-100k|*-pages-100|*-slides-50) want=M ;;
    *-1m|*-pages-500|*-slides-200) want=L ;;
    *) want= ;;
  esac
  for t in "${tier_arr[@]}"; do
    if [ "$t" = "$want" ]; then selected_ids+=("$id"); break; fi
  done
done
if [ "${#selected_ids[@]}" -eq 0 ]; then
  echo "perf_beats_ts.sh: no fixtures for tiers=$TIERS format=$fmt" >&2
  exit 2
fi
echo "perf_beats_ts.sh: generating fixtures: ${selected_ids[*]}"
python3 "$ROOT/fixtures/gen/generate.py" $(printf -- '--id %s ' "${selected_ids[@]}")

# --- cold openDocument (ts-vs-wasm) ---
OPEN_JSON="$OUT_DIR/ts-vs-wasm.json"
echo "perf_beats_ts.sh: openDocument timing → $OPEN_JSON"
node --expose-gc "$ROOT/perf/large/ts-vs-wasm.mjs" \
  --format="$fmt" \
  --tier="$TIERS" \
  --runs="$RUNS_OPEN" \
  --warmup="$WARMUP_OPEN" \
  --bootstrap="$BOOTSTRAP" \
  --json > "$OPEN_JSON"

# --- peak memory (VmHWM growth, fresh process per sample) ---
MEM_JSON="$OUT_DIR/peak-mem.json"
echo "perf_beats_ts.sh: peak memory → $MEM_JSON"
node "$ROOT/perf/large/peak-mem.mjs" \
  --format="$fmt" \
  --tier="$TIERS" \
  --runs="$RUNS_MEM" \
  --bootstrap="$BOOTSTRAP" \
  --json > "$MEM_JSON"

# --- per-test table + pass/fail ---
SUMMARY_MD="$OUT_DIR/summary.md"
FAILS_FILE="$OUT_DIR/fails.txt"
READINESS_JSON="$OUT_DIR/enforce-readiness.json"
: > "$FAILS_FILE"

node - "$OPEN_JSON" "$MEM_JSON" "$SUMMARY_MD" "$FAILS_FILE" "$READINESS_JSON" "$fmt" "$MODE" "$arm_issue" <<'JS'
const fs = require("fs");
const [openPath, memPath, outMd, failsPath, readinessPath, fmt, mode, armIssue] =
  process.argv.slice(2);
const open = JSON.parse(fs.readFileSync(openPath, "utf8"));
const mem = JSON.parse(fs.readFileSync(memPath, "utf8"));

function fmtN(n, d = 3) {
  if (!Number.isFinite(n)) return "—";
  return n.toFixed(d);
}

const fails = [];
const openFails = [];
const peakFails = [];
const lines = [];
lines.push(`## WASM-beats-TS — \`${fmt}\` (**${mode.toUpperCase()} MODE**)`);
lines.push("");
if (mode === "report") {
  lines.push(
    `> **REPORT MODE** — measuring and posting numbers; job stays green on LOSS.`,
  );
  lines.push(
    `> Bar unchanged (median &lt; 1 **and** 95% CI upper &lt; 1). Jared: set \`PERF_GATE_MODE=enforce\` to arm (see ${armIssue} / #8).`,
  );
  lines.push("");
} else {
  lines.push(
    `> **ENFORCE MODE** — any LOSS / TIE/LOSS fails this job. Pass rule: median WASM/TS &lt; 1 and 95% CI upper &lt; 1.`,
  );
  lines.push("");
}
lines.push(
  `| Format | Tier | Metric | TS median | WASM median | WASM/TS | 95% CI | Verdict |`,
);
lines.push(
  `|---|---|---|---:|---:|---:|---:|---|`,
);

for (const r of open.rows || []) {
  const ci = r.wasm_over_ts_ci95 || {};
  const v = r.verdict || "LOSS";
  lines.push(
    `| ${r.format} | ${r.tier} | openDocument (ms) | ${fmtN(r.ts_ms_median, 2)} | ${fmtN(r.wasm_ms_median, 2)} | ${fmtN(r.wasm_over_ts_median)} | [${fmtN(ci.lo)}, ${fmtN(ci.hi)}] | **${v}** |`,
  );
  if (v !== "WIN") {
    const label = `${r.format} ${r.tier} openDocument ${v}`;
    fails.push(label);
    openFails.push(label);
  }
}

for (const r of mem.rows || []) {
  const ci = r.wasm_over_ts_mem_ci95 || {};
  const v = r.verdict || "LOSS";
  lines.push(
    `| ${r.format} | ${r.tier} | peak mem (MiB) | ${fmtN(r.ts.mem, 1)} | ${fmtN(r.wasm.mem, 1)} | ${fmtN(r.wasm_over_ts_mem)} | [${fmtN(ci.lo)}, ${fmtN(ci.hi)}] | **${v}** |`,
  );
  if (v !== "WIN") {
    const label = `${r.format} ${r.tier} peak_mem ${v}`;
    fails.push(label);
    peakFails.push(label);
  }
}

const wins = (open.rows || []).length + (mem.rows || []).length - fails.length;
const total = (open.rows || []).length + (mem.rows || []).length;
const enforceReady = fails.length === 0;
lines.push("");
lines.push(
  `**Result:** ${wins}/${total} WIN · ${fails.length} LOSS/TIE · mode=\`${mode}\``,
);
if (fails.length) {
  lines.push("");
  lines.push("Losses (shown as prominently as wins):");
  for (const f of fails) lines.push(`- ${f}`);
}
lines.push("");
lines.push("### Enforce readiness");
lines.push("");
if (enforceReady) {
  lines.push(
    `**READY** — all ${total} cells WIN. Jared may set \`PERF_GATE_MODE=enforce\` for this format (see ${armIssue} / #8) and keep \`perf-beats-ts\` in branch-protection required checks. Agents do not edit workflows or branch protection.`,
  );
} else {
  lines.push(
    `**NOT READY** — flipping \`PERF_GATE_MODE=enforce\` would fail this job. Bar unchanged (no thresholds lowered).`,
  );
  if (openFails.length === 0) {
    lines.push(
      `- openDocument: all WIN (time gate clear for this format)`,
    );
  } else {
    lines.push(`- openDocument blockers: ${openFails.join("; ")}`);
  }
  if (peakFails.length) {
    lines.push(
      `- peak_mem blockers: ${peakFails.join("; ")} (xlsx/docx S often wasm compile floor — see \`docs/M1-REPORT.md\` §5.4)`,
    );
  }
}
lines.push("");
lines.push(
  `Method: open = \`perf/large/ts-vs-wasm.mjs\` (Handle.openDocument vs upstream load*); peak mem = \`perf/large/peak-mem.mjs\` (VmHWM growth, fresh process). Ratio = median(wasm)/median(ts); 95% CI unpaired bootstrap.`,
);

const readiness = {
  format: fmt,
  mode,
  armIssue,
  enforceReady,
  wins,
  total,
  losses: fails.length,
  openDocumentBlockers: openFails,
  peakMemBlockers: peakFails,
  blockers: fails,
};

fs.writeFileSync(outMd, lines.join("\n") + "\n");
fs.writeFileSync(failsPath, fails.join("\n") + (fails.length ? "\n" : ""));
fs.writeFileSync(readinessPath, JSON.stringify(readiness, null, 2) + "\n");
process.stdout.write(lines.join("\n") + "\n");
JS

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$SUMMARY_MD" >> "$GITHUB_STEP_SUMMARY"
fi

fail_count=$(wc -l < "$FAILS_FILE" | tr -d ' ')
if [ "${fail_count:-0}" -gt 0 ]; then
  echo "perf_beats_ts.sh: ${fail_count} LOSS/TIE (see table above)" >&2
  echo "perf_beats_ts.sh: enforce readiness NOT READY ($READINESS_JSON)" >&2
  if [ "$MODE" = "enforce" ]; then
    echo "perf_beats_ts.sh: ENFORCE MODE — failing job" >&2
    exit 1
  fi
  echo "perf_beats_ts.sh: REPORT MODE — exiting 0; do not flip PERF_GATE_MODE=enforce until readiness is READY" >&2
  exit 0
fi

echo "perf_beats_ts.sh: all cells WIN (mode=$MODE) — enforce readiness READY ($READINESS_JSON)"
exit 0
