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
#   PERF_GATE_MODE=enforce  — same table; exit 1 on any non-excepted LOSS / TIE/LOSS.
#
# Harnesses: perf/large/ts-vs-wasm.mjs (cold openDocument) and perf/large/peak-mem.mjs
# (fresh-process VmHWM). Fixtures generated in-job. XL / 4× CPU / scroll / browser first-paint
# remain TODO(#8) once the Node open+peak baseline is green.
#
# Known-loss exceptions (M1 compile floor; Jared 2026-10-22):
#   https://github.com/jgridifier/nitro-xml/issues/30#issuecomment-6341154384
#   xlsx S peak_mem, docx S peak_mem, docx M peak_mem — stay measured and printed as LOSS
#   with numbers; they do not block enforce readiness or fail ENFORCE mode. Any other cell
#   that is LOSS / TIE/LOSS still fails. No fixture demotion; no threshold/budget changes.
#
# Definition of done for arming: enforce-readiness READY (all cells WIN, or only the
# documented known-loss exceptions above), then Jared flips PERF_GATE_MODE=enforce (or
# sets it in the workflow env) and asks Jared / Chief of Staff to keep `perf-beats-ts`
# in branch-protection required checks. Agents never edit .github/workflows/.
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

node - "$OPEN_JSON" "$MEM_JSON" "$SUMMARY_MD" "$FAILS_FILE" "$READINESS_JSON" "$fmt" "$MODE" "$arm_issue" "$ROOT" <<'JS'
const fs = require("fs");
const path = require("path");
const [openPath, memPath, outMd, failsPath, readinessPath, fmt, mode, armIssue, root] =
  process.argv.slice(2);
const {
  KNOWN_LOSS_DECISION,
  classifyMeasuredLoss,
} = require(path.join(root, "scripts/ci/lib/perf-beats-known-loss.cjs"));
const open = JSON.parse(fs.readFileSync(openPath, "utf8"));
const mem = JSON.parse(fs.readFileSync(memPath, "utf8"));

function fmtN(n, d = 3) {
  if (!Number.isFinite(n)) return "—";
  return n.toFixed(d);
}

const measuredLosses = [];
const blockingFails = [];
const knownLosses = [];
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
    `> **ENFORCE MODE** — any non-excepted LOSS / TIE/LOSS fails this job. Documented known-loss cells stay printed as LOSS but do not fail. Pass rule: median WASM/TS &lt; 1 and 95% CI upper &lt; 1.`,
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
    const cls = classifyMeasuredLoss(r.format, r.tier, "openDocument", v);
    measuredLosses.push(cls.label);
    if (cls.blocking) {
      blockingFails.push(cls.label);
      openFails.push(cls.label);
    }
  }
}

for (const r of mem.rows || []) {
  const ci = r.wasm_over_ts_mem_ci95 || {};
  const v = r.verdict || "LOSS";
  const tsMem = r.ts && Number.isFinite(r.ts.mem) ? r.ts.mem : NaN;
  const wasmMem = r.wasm && Number.isFinite(r.wasm.mem) ? r.wasm.mem : NaN;
  lines.push(
    `| ${r.format} | ${r.tier} | peak mem (MiB) | ${fmtN(tsMem, 1)} | ${fmtN(wasmMem, 1)} | ${fmtN(r.wasm_over_ts_mem)} | [${fmtN(ci.lo)}, ${fmtN(ci.hi)}] | **${v}** |`,
  );
  if (v !== "WIN") {
    const cls = classifyMeasuredLoss(r.format, r.tier, "peak_mem", v);
    measuredLosses.push(cls.label);
    if (cls.knownLoss) {
      knownLosses.push({
        format: r.format,
        tier: r.tier,
        metric: "peak_mem",
        verdict: v,
        label: cls.label,
        tsMiB: tsMem,
        wasmMiB: wasmMem,
        wasmOverTs: r.wasm_over_ts_mem,
        ci95: ci,
        decision: KNOWN_LOSS_DECISION,
      });
    } else {
      blockingFails.push(cls.label);
      peakFails.push(cls.label);
    }
  }
}

const wins =
  (open.rows || []).length + (mem.rows || []).length - measuredLosses.length;
const total = (open.rows || []).length + (mem.rows || []).length;
const enforceReady = blockingFails.length === 0;
lines.push("");
lines.push(
  `**Result:** ${wins}/${total} WIN · ${measuredLosses.length} LOSS/TIE · ${knownLosses.length} known-loss exception · ${blockingFails.length} blocking · mode=\`${mode}\``,
);
if (measuredLosses.length) {
  lines.push("");
  lines.push("Losses (shown as prominently as wins; numbers kept):");
  for (const f of measuredLosses) {
    const known = knownLosses.some((k) => k.label === f);
    lines.push(known ? `- ${f} _(known-loss exception — still LOSS)_` : `- ${f}`);
  }
}
if (knownLosses.length) {
  lines.push("");
  lines.push("### Known-loss exceptions (M1 compile floor)");
  lines.push("");
  lines.push(
    `Documented by Jared ([comment](${KNOWN_LOSS_DECISION})): cells stay **LOSS** with measured numbers; they do not block enforce readiness and do not fail ENFORCE mode. No fixture demotion; thresholds and \`perf/budgets.json\` unchanged. Any other LOSS / TIE/LOSS still fails.`,
  );
  lines.push("");
  lines.push(
    `| Format | Tier | Metric | TS MiB | WASM MiB | WASM/TS | Verdict |`,
  );
  lines.push(`|---|---|---|---:|---:|---:|---|`);
  for (const k of knownLosses) {
    const lo = k.ci95 && Number.isFinite(k.ci95.lo) ? fmtN(k.ci95.lo) : "—";
    const hi = k.ci95 && Number.isFinite(k.ci95.hi) ? fmtN(k.ci95.hi) : "—";
    lines.push(
      `| ${k.format} | ${k.tier} | ${k.metric} | ${fmtN(k.tsMiB, 1)} | ${fmtN(k.wasmMiB, 1)} | ${fmtN(k.wasmOverTs)} [${lo}, ${hi}] | **${k.verdict}** |`,
    );
  }
}
lines.push("");
lines.push("### Enforce readiness");
lines.push("");
if (enforceReady) {
  if (knownLosses.length === 0) {
    lines.push(
      `**READY** — all ${total} cells WIN. Jared may set \`PERF_GATE_MODE=enforce\` for this format (see ${armIssue} / #8) and keep \`perf-beats-ts\` in branch-protection required checks. Agents do not edit workflows or branch protection.`,
    );
  } else {
    lines.push(
      `**READY** — ${wins}/${total} WIN; ${knownLosses.length} documented known-loss exception(s) still printed as LOSS ([Jared](${KNOWN_LOSS_DECISION})). Jared may set \`PERF_GATE_MODE=enforce\` for this format (see ${armIssue} / #8). Agents do not edit workflows or branch protection.`,
    );
  }
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
    lines.push(`- peak_mem blockers: ${peakFails.join("; ")}`);
  }
  if (knownLosses.length) {
    lines.push(
      `- known-loss exceptions (not blockers): ${knownLosses.map((k) => k.label).join("; ")} ([decision](${KNOWN_LOSS_DECISION}))`,
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
  losses: measuredLosses.length,
  blockingLosses: blockingFails.length,
  knownLossExceptions: knownLosses,
  knownLossDecision: KNOWN_LOSS_DECISION,
  openDocumentBlockers: openFails,
  peakMemBlockers: peakFails,
  blockers: blockingFails,
  measuredLosses,
};

fs.writeFileSync(outMd, lines.join("\n") + "\n");
// FAILS_FILE drives exit code: only blocking (non-excepted) losses.
fs.writeFileSync(
  failsPath,
  blockingFails.join("\n") + (blockingFails.length ? "\n" : ""),
);
fs.writeFileSync(readinessPath, JSON.stringify(readiness, null, 2) + "\n");
process.stdout.write(lines.join("\n") + "\n");
JS

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$SUMMARY_MD" >> "$GITHUB_STEP_SUMMARY"
fi

fail_count=$(wc -l < "$FAILS_FILE" | tr -d ' ')
if [ "${fail_count:-0}" -gt 0 ]; then
  echo "perf_beats_ts.sh: ${fail_count} blocking LOSS/TIE (see table above)" >&2
  echo "perf_beats_ts.sh: enforce readiness NOT READY ($READINESS_JSON)" >&2
  if [ "$MODE" = "enforce" ]; then
    echo "perf_beats_ts.sh: ENFORCE MODE — failing job" >&2
    exit 1
  fi
  echo "perf_beats_ts.sh: REPORT MODE — exiting 0; do not flip PERF_GATE_MODE=enforce until readiness is READY" >&2
  exit 0
fi

echo "perf_beats_ts.sh: enforce readiness READY ($READINESS_JSON) — mode=$MODE (known-loss exceptions may still print as LOSS)"
exit 0
