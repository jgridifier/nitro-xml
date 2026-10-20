#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Perf track. Usage:
#   perf.sh gate   — PR job (perf-gate): S/M/L × xlsx/docx/pptx openDocument + peak mem
#   perf.sh record — main/nightly/dispatch (perf-record): same measurements, write JSON under
#                    /tmp (perf-data branch append + 4× CPU + XL remain TODO(#8))
#
# Every commit posts its numbers to the job summary. Default PERF_GATE_MODE=report so the
# job stays green while losses are still measured and shown; set PERF_GATE_MODE=enforce to
# fail on any LOSS / TIE/LOSS (PRODUCT_SPEC §5 bar unchanged).
#
# Definition of done for #8 history/thresholds slice: append to perf-data and wire
# regression-vs-main. This script already deletes the stub and runs the WASM-beats-TS
# measurements via perf_beats_ts.sh. Agents never edit .github/workflows/.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

mode="${1:-gate}"
case "$mode" in gate|record) ;; *)
  echo "perf.sh: usage: perf.sh gate|record" >&2
  exit 2
esac

MODE="${PERF_GATE_MODE:-report}"
TIERS="${PERF_BEATS_TIERS:-S,M,L}"
SHA=$(git rev-parse HEAD)
SHORT=$(git rev-parse --short HEAD)
OUT_DIR="${PERF_OUT:-/tmp/perf-$mode-$SHORT}"
mkdir -p "$OUT_DIR"

echo "perf.sh mode=$mode gate_mode=$MODE tiers=$TIERS sha=$SHA"
echo "out=$OUT_DIR"

# Combined summary header
{
  echo "## perf.sh \`$mode\` (**${MODE^^} MODE**)"
  echo ""
  if [ "$MODE" = "report" ]; then
    echo "> **REPORT MODE** — numbers posted; LOSS does not fail the job. Bar unchanged (CI upper &lt; 1.0). Jared: \`PERF_GATE_MODE=enforce\` to arm (#8)."
  else
    echo "> **ENFORCE MODE** — any LOSS / TIE/LOSS from \`perf_beats_ts.sh\` fails this job."
  fi
  echo ""
  echo "Tiers: \`$TIERS\` · formats: xlsx, docx, pptx · metrics: openDocument time + peak VmHWM"
  echo ""
} | tee "$OUT_DIR/header.md"

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$OUT_DIR/header.md" >> "$GITHUB_STEP_SUMMARY"
fi

overall_status=0
for fmt in xlsx docx pptx; do
  fmt_out="$OUT_DIR/$fmt"
  mkdir -p "$fmt_out"
  echo "perf.sh: running perf_beats_ts.sh $fmt → $fmt_out"
  # Always inherit PERF_GATE_MODE so report/enforce is consistent across formats.
  if ! PERF_BEATS_OUT="$fmt_out" PERF_BEATS_TIERS="$TIERS" \
      bash "$ROOT/scripts/ci/perf_beats_ts.sh" "$fmt"; then
    overall_status=1
  fi
  # Copy per-format JSONs into the aggregate out dir (already under fmt_out).
  if [ -f "$fmt_out/ts-vs-wasm.json" ]; then
    cp "$fmt_out/ts-vs-wasm.json" "$OUT_DIR/${fmt}-ts-vs-wasm.json"
  fi
  if [ -f "$fmt_out/peak-mem.json" ]; then
    cp "$fmt_out/peak-mem.json" "$OUT_DIR/${fmt}-peak-mem.json"
  fi
done

# Aggregate win/loss rollup for the job summary
node - "$OUT_DIR" <<'JS' | tee "$OUT_DIR/rollup.md"
const fs = require("fs");
const path = require("path");
const outDir = process.argv[2];
const rows = [];
for (const fmt of ["xlsx", "docx", "pptx"]) {
  const openPath = path.join(outDir, `${fmt}-ts-vs-wasm.json`);
  const memPath = path.join(outDir, `${fmt}-peak-mem.json`);
  if (fs.existsSync(openPath)) {
    const open = JSON.parse(fs.readFileSync(openPath, "utf8"));
    for (const r of open.rows || []) {
      rows.push({
        format: r.format,
        tier: r.tier,
        metric: "openDocument",
        ratio: r.wasm_over_ts_median,
        ci: r.wasm_over_ts_ci95,
        verdict: r.verdict,
      });
    }
  }
  if (fs.existsSync(memPath)) {
    const mem = JSON.parse(fs.readFileSync(memPath, "utf8"));
    for (const r of mem.rows || []) {
      rows.push({
        format: r.format,
        tier: r.tier,
        metric: "peak_mem",
        ratio: r.wasm_over_ts_mem,
        ci: r.wasm_over_ts_mem_ci95,
        verdict: r.verdict,
      });
    }
  }
}
const wins = rows.filter((r) => r.verdict === "WIN").length;
const losses = rows.filter((r) => r.verdict !== "WIN");
const lines = [];
lines.push("### perf.sh rollup");
lines.push("");
lines.push(`**${wins}/${rows.length} WIN** · **${losses.length} LOSS/TIE**`);
lines.push("");
if (losses.length) {
  lines.push("| Format | Tier | Metric | WASM/TS | 95% CI | Verdict |");
  lines.push("|---|---|---|---:|---:|---|");
  for (const r of losses) {
    const lo = r.ci && Number.isFinite(r.ci.lo) ? r.ci.lo.toFixed(3) : "—";
    const hi = r.ci && Number.isFinite(r.ci.hi) ? r.ci.hi.toFixed(3) : "—";
    const ratio = Number.isFinite(r.ratio) ? r.ratio.toFixed(3) : "—";
    lines.push(`| ${r.format} | ${r.tier} | ${r.metric} | ${ratio} | [${lo}, ${hi}] | **${r.verdict}** |`);
  }
  lines.push("");
}
lines.push(
  "TODO(#8): perf-data branch append, regression-vs-main thresholds, 4× CPU, XL, scroll/view-query, browser first-paint (fetch/compile/instantiate).",
);
process.stdout.write(lines.join("\n") + "\n");
JS

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  cat "$OUT_DIR/rollup.md" >> "$GITHUB_STEP_SUMMARY"
fi

if [ "$mode" = "record" ]; then
  # Persist JSON bundle for later perf-data ingestion (append not wired yet — TODO(#8)).
  manifest="$OUT_DIR/record-manifest.json"
  node -e '
    const fs = require("fs");
    const out = process.argv[1];
    const sha = process.argv[2];
    const files = fs.readdirSync(out).filter((f) => f.endsWith(".json"));
    fs.writeFileSync(out + "/record-manifest.json", JSON.stringify({
      schema: 1,
      mode: "record",
      sha,
      when: new Date().toISOString(),
      note: "TODO(#8): append to perf-data branch; this artifact is local/CI-only for now",
      files,
    }, null, 2) + "\n");
  ' "$OUT_DIR" "$SHA"
  echo "perf.sh record: wrote $manifest (perf-data append TODO(#8))"
fi

if [ "$overall_status" -ne 0 ]; then
  echo "perf.sh: ENFORCE MODE — one or more formats failed" >&2
  exit 1
fi

echo "perf.sh: done (mode=$mode gate_mode=$MODE)"
exit 0
