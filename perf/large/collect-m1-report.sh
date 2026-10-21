#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Collect M1-report metrics from existing harnesses (docs/M1-REPORT.md §5–§7).
#
# Does NOT flip PERF_GATE_MODE=enforce or edit budgets/workflows. TS-vs-WASM
# cold-open tables come from perf/large/ts-vs-wasm.mjs. perf_beats_ts.sh /
# perf.sh are live in report mode (#342); scoreboard.sh may still stub (#39).
#
# Usage (from repo root):
#   bash perf/large/collect-m1-report.sh
#   bash perf/large/collect-m1-report.sh --skip-generate   # fixtures already present
#   bash perf/large/collect-m1-report.sh --skip-build-wasm # pkg already matches tip
#   bash perf/large/collect-m1-report.sh --skip-ts-vs-wasm # native/wasm/bundle only
#
# Writes logs under /tmp/m1-metrics-<shortsha>/; do not commit L/XL binaries.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

SKIP_GENERATE=0
SKIP_BUILD_WASM=0
SKIP_TS_VS_WASM=0
RUNS_NATIVE=7
RUNS_WASM=7
RUNS_COMPARE=11
for a in "$@"; do
  case "$a" in
    --skip-generate) SKIP_GENERATE=1 ;;
    --skip-build-wasm) SKIP_BUILD_WASM=1 ;;
    --skip-ts-vs-wasm) SKIP_TS_VS_WASM=1 ;;
    --runs=*) RUNS_NATIVE="${a#--runs=}"; RUNS_WASM="$RUNS_NATIVE"; RUNS_COMPARE="$RUNS_NATIVE" ;;
    *) echo "unknown arg: $a" >&2; exit 2 ;;
  esac
done

SHA=$(git rev-parse HEAD)
SHORT=$(git rev-parse --short HEAD)
OUT="/tmp/m1-metrics-$SHORT"
mkdir -p "$OUT"
HOST=$(uname -a)
WHEN=$(date -u +%Y-%m-%dT%H:%M:%SZ)

echo "m1-report-metrics sha=$SHA short=$SHORT when=$WHEN"
echo "host=$HOST"
echo "out=$OUT"

# --- Gate / deferred harnesses (record exactly what they print) ---
# perf_beats_ts / perf.sh are real but default REPORT MODE (exit 0 on LOSS) — #342.
# Full S/M/L × 3 formats is slow; collector keeps a short S-only sample + scoreboard stub.
{
  echo "### gates"
  echo "--- perf_beats_ts.sh xlsx (PERF_BEATS_TIERS=S, report) ---"
  PERF_GATE_MODE=report PERF_BEATS_TIERS=S bash scripts/ci/perf_beats_ts.sh xlsx || true
  echo "--- scoreboard.sh ---"
  bash scripts/ci/scoreboard.sh || true
} | tee "$OUT/gates.txt"

# --- Fixtures (on-demand L/XL; S/M usually already committed under fixtures/generated) ---
if [ "$SKIP_GENERATE" -eq 0 ]; then
  python3 fixtures/gen/generate.py \
    --id gen-xlsx-cells-1k \
    --id gen-xlsx-cells-100k \
    --id gen-xlsx-cells-1m \
    --id gen-xlsx-xl-a \
    --id gen-docx-pages-10 \
    --id gen-docx-pages-100 \
    --id gen-docx-pages-500 \
    --id gen-docx-xl-a \
    --id gen-pptx-slides-10 \
    --id gen-pptx-slides-50 \
    --id gen-pptx-slides-200 \
    --id gen-pptx-xl-a \
    | tee "$OUT/generate.txt"
fi

# --- Native large baseline (one process per fixture for clean VmHWM) ---
cargo build --release --manifest-path perf/large/Cargo.toml >/dev/null
{
  echo "sha=$SHA runs=$RUNS_NATIVE"
  # xlsx: --xlsx-small labels as xlsx_100k; --xlsx as xlsx_1m (harness label quirk).
  cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
    --xlsx-small fixtures/generated/xlsx/cells_1k.xlsx --runs "$RUNS_NATIVE"
  cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
    --xlsx-small fixtures/generated/xlsx/cells_100k.xlsx --runs "$RUNS_NATIVE"
  cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
    --xlsx fixtures/generated/xlsx/cells_1m.xlsx --runs "$RUNS_NATIVE"
  cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
    --xlsx fixtures/generated/xlsx/xl_a.xlsx --runs "$RUNS_NATIVE"
  for f in \
    fixtures/generated/docx/pages_10.docx \
    fixtures/generated/docx/pages_100.docx \
    fixtures/generated/docx/pages_500.docx \
    fixtures/generated/docx/xl_a.docx
  do
    cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
      --docx "$f" --runs "$RUNS_NATIVE"
  done
  for f in \
    fixtures/generated/pptx/slides_10.pptx \
    fixtures/generated/pptx/slides_50.pptx \
    fixtures/generated/pptx/slides_200.pptx \
    fixtures/generated/pptx/xl_a.pptx
  do
    cargo run --release --quiet --manifest-path perf/large/Cargo.toml -- \
      --pptx "$f" --runs "$RUNS_NATIVE"
  done
} | tee "$OUT/native.txt"

# --- WASM open baseline ---
if [ "$SKIP_BUILD_WASM" -eq 0 ]; then
  bash scripts/build-wasm.sh | tee "$OUT/build-wasm.txt" | tail -20
fi
{
  echo "sha=$SHA runs=$RUNS_WASM"
  for f in \
    fixtures/generated/xlsx/cells_1k.xlsx \
    fixtures/generated/xlsx/cells_100k.xlsx \
    fixtures/generated/xlsx/cells_1m.xlsx \
    fixtures/generated/xlsx/xl_a.xlsx \
    fixtures/generated/docx/pages_10.docx \
    fixtures/generated/docx/pages_100.docx \
    fixtures/generated/docx/pages_500.docx \
    fixtures/generated/docx/xl_a.docx \
    fixtures/generated/pptx/slides_10.pptx \
    fixtures/generated/pptx/slides_50.pptx \
    fixtures/generated/pptx/slides_200.pptx \
    fixtures/generated/pptx/xl_a.pptx
  do
    echo "=== $f ==="
    node perf/large/wasm-baseline.mjs "$f" "$RUNS_WASM"
  done
} | tee "$OUT/wasm.txt"

# --- Bundle / loader ---
{
  echo "sha=$SHA"
  bash scripts/ci/bundle_budget.sh
  node -e '
    const fs=require("fs"), zlib=require("zlib");
    const w=fs.readFileSync("packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm");
    const j=fs.readFileSync("packages/nitro-xml/pkg/nitro_xml_wasm.js");
    const b=zlib.brotliCompressSync(w,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11}});
    console.log(JSON.stringify({
      wasm_raw_bytes: w.length,
      wasm_glue_js_bytes: j.length,
      wasm_brotli_q11_bytes: b.length,
    }));
  '
} | tee "$OUT/bundle.txt"

# --- TS vs WASM cold open (report-only; does not arm perf_beats_ts) ---
if [ "$SKIP_TS_VS_WASM" -eq 0 ]; then
  {
    echo "sha=$SHA"
    node --expose-gc perf/large/ts-vs-wasm.mjs --runs="$RUNS_COMPARE" --warmup=2 --bootstrap=2000
  } | tee "$OUT/ts-vs-wasm.txt"
fi

# --- Compact summary JSON (pointers + gate status; numbers live in the teed logs) ---
node - "$OUT" "$SHA" "$SHORT" "$WHEN" "$HOST" <<'JS'
const fs = require('fs');
const [out, sha, short, when, host] = process.argv.slice(2);
const gates = fs.readFileSync(`${out}/gates.txt`, 'utf8');
const hasCompare = fs.existsSync(`${out}/ts-vs-wasm.txt`);
const summary = {
  sha,
  short,
  when,
  host,
  out,
  commands: [
    'bash perf/large/collect-m1-report.sh',
    'node --expose-gc perf/large/ts-vs-wasm.mjs',
    'PERF_GATE_MODE=report bash scripts/ci/perf_beats_ts.sh <xlsx|docx|pptx>',
    'bash scripts/ci/scoreboard.sh',
    'bash scripts/ci/bundle_budget.sh',
  ],
  gates: {
    perf_beats_ts_not_implemented: /NOT IMPLEMENTED/.test(gates),
    perf_beats_ts_report_mode: /REPORT MODE/.test(gates),
    scoreboard_sh_stub: /NOT IMPLEMENTED — see #39/.test(gates),
    raw: gates.trim().split('\n'),
  },
  gaps: {
    ts_vs_wasm_gate_enforce:
      'Gate scripts are live in PERF_GATE_MODE=report (#342 / docs/M1-REPORT.md §5.4). Jared flips to enforce when reds are fixed; docx arm deferred under #23.',
    scoreboard_standings_including_losses:
      'Unavailable without scoreboard render-race fix: issue #41 and draft PR #113. nitro-xml remains "not entered" in docs/scoreboard/data/parse-latest.json. Do not invent standings.',
  },
  logs: {
    native: `${out}/native.txt`,
    wasm: `${out}/wasm.txt`,
    bundle: `${out}/bundle.txt`,
    gates: `${out}/gates.txt`,
    ts_vs_wasm: hasCompare ? `${out}/ts-vs-wasm.txt` : null,
  },
};
fs.writeFileSync(`${out}/summary.json`, JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
JS

echo "m1-report-metrics done → $OUT/summary.json"
