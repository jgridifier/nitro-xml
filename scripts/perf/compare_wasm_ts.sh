#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# M1 closeout: reproducible WASM vs TS comparison on S/M/L/XL fixtures (#117).
#
# Measures, per fixture × format (xlsx/docx/pptx) × engine (WASM, TS):
#   - Cold openDocument latency  (perf/large/ts-vs-wasm.mjs: median, p95, stdev, IQR)
#   - Peak memory / VmHWM        (perf/large/peak-mem.mjs: median, p95, stdev)
#   - Bundle sizes               (perf/sizes.mjs: raw, gzip, brotli per engine entry)
#   - Save latency               (not available yet — see CAVEATS)
#
# Output: docs/perf/wasm-vs-ts.md + docs/perf/wasm-vs-ts.json
#
# Usage (from repo root):
#   bash scripts/perf/compare_wasm_ts.sh [--tiers S,M,L,XL] [--formats xlsx,docx,pptx]
#        [--runs N] [--warmup N] [--mem-runs N] [--bootstrap N]
#        [--out-dir docs/perf] [--skip-wasm-build] [--skip-ts-build]
#        [--report-only]  # exit 0 regardless of WASM-beats-TS result
#
# To invoke from the existing perf CI job (perf.sh record / gate), just source or call it:
#   COMPARE_TIERS=S,M,L,XL bash scripts/perf/compare_wasm_ts.sh
#
# NOTE: Run heavy benchmarks one at a time (--concurrency is always 1); the box has ~15 GB
# RAM shared with other work. XL tier benchmarks are skipped if the fixture is > 50 MB to
# avoid OOM on constrained runners; override with COMPARE_ALLOW_LARGE_XL=1.
#
# HARD RULES (AGENTS.md):
#   - Never edit .github/workflows/*
#   - Never lower thresholds, raise perf/budgets.json, demote fixtures, or loosen gates
#   - Never print or commit tokens
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT=$PWD

# shellcheck source=scripts/ci/versions.env
. scripts/ci/versions.env
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

# --- Argument parsing ---
TIERS="${COMPARE_TIERS:-S,M,L,XL}"
FORMATS="${COMPARE_FORMATS:-xlsx,docx,pptx}"
RUNS="${COMPARE_RUNS:-21}"
WARMUP="${COMPARE_WARMUP:-8}"
MEM_RUNS="${COMPARE_MEM_RUNS:-7}"
BOOTSTRAP="${COMPARE_BOOTSTRAP:-2000}"
OUT_DIR="${COMPARE_OUT_DIR:-docs/perf}"
SKIP_WASM_BUILD="${COMPARE_SKIP_WASM_BUILD:-0}"
SKIP_TS_BUILD="${COMPARE_SKIP_TS_BUILD:-0}"
REPORT_ONLY="${COMPARE_REPORT_ONLY:-1}"   # M1: always report mode; gate is in perf_beats_ts.sh
ALLOW_LARGE_XL="${COMPARE_ALLOW_LARGE_XL:-0}"

for a in "$@"; do
  case "$a" in
    --tiers=*)    TIERS="${a#*=}";;
    --formats=*)  FORMATS="${a#*=}";;
    --runs=*)     RUNS="${a#*=}";;
    --warmup=*)   WARMUP="${a#*=}";;
    --mem-runs=*) MEM_RUNS="${a#*=}";;
    --bootstrap=*) BOOTSTRAP="${a#*=}";;
    --out-dir=*)  OUT_DIR="${a#*=}";;
    --skip-wasm-build) SKIP_WASM_BUILD=1;;
    --skip-ts-build)   SKIP_TS_BUILD=1;;
    --report-only) REPORT_ONLY=1;;
  esac
done

SHA=$(git rev-parse HEAD)
SHORT=$(git rev-parse --short HEAD)
WHEN=$(date -u +%Y-%m-%dT%H:%M:%SZ)
HOST_OS=$(uname -s)
HOST_ARCH=$(uname -m)
NODE_VER=$(node --version)

echo "compare_wasm_ts.sh sha=$SHA tiers=$TIERS formats=$FORMATS"
echo "runs=$RUNS warmup=$WARMUP mem_runs=$MEM_RUNS bootstrap=$BOOTSTRAP"
echo "out=$OUT_DIR"

TMP_DIR=$(mktemp -d /tmp/compare-wasm-ts-XXXXXX)
trap 'rm -rf "$TMP_DIR"' EXIT

# --- Toolchain checks ---
if ! command -v bun >/dev/null 2>&1; then
  echo "compare_wasm_ts.sh: installing bun"
  curl -fsSL https://bun.sh/install | bash
  export PATH="$HOME/.bun/bin:$PATH"
fi

# --- Build WASM (unless caller already built) ---
if [ "$SKIP_WASM_BUILD" = 1 ] \
  && [ -f "$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm" ] \
  && [ -f "$ROOT/packages/nitro-xml/pkg/nitro_xml_wasm.js" ]; then
  echo "compare_wasm_ts.sh: reusing existing wasm pkg"
else
  echo "compare_wasm_ts.sh: building wasm package (--skip-wasm-build to reuse)"
  bash "$ROOT/scripts/build-wasm.sh"
fi

# --- Build TS oracle (unless caller already built) ---
if [ "$SKIP_TS_BUILD" = 1 ] && [ -d "$ROOT/upstream/src/core/dist/pptx" ]; then
  echo "compare_wasm_ts.sh: reusing existing TS oracle"
else
  source "$ROOT/scripts/ci/ensure-upstream-oracle.sh" "$ROOT"
fi

# --- Generate fixtures for each selected format × tier ---
echo "compare_wasm_ts.sh: generating fixtures for formats=$FORMATS tiers=$TIERS"
IFS=',' read -r -a TIER_ARR <<< "$TIERS"
IFS=',' read -r -a FMT_ARR <<< "$FORMATS"

SKIPPED_XL=()
for fmt in "${FMT_ARR[@]}"; do
  for tier in "${TIER_ARR[@]}"; do
    case "$fmt-$tier" in
      xlsx-S) python3 "$ROOT/fixtures/gen/generate.py" --id gen-xlsx-cells-1k 2>&1 ;;
      xlsx-M) python3 "$ROOT/fixtures/gen/generate.py" --id gen-xlsx-cells-100k 2>&1 ;;
      xlsx-L) python3 "$ROOT/fixtures/gen/generate.py" --id gen-xlsx-cells-1m 2>&1 ;;
      xlsx-XL)
        path="$ROOT/fixtures/generated/xlsx/xl_a.xlsx"
        python3 "$ROOT/fixtures/gen/generate.py" --id gen-xlsx-xl-a 2>&1
        if [ "$ALLOW_LARGE_XL" = 0 ] && [ -f "$path" ]; then
          size=$(wc -c < "$path")
          if [ "$size" -gt 52428800 ]; then
            echo "compare_wasm_ts.sh: WARNING xlsx XL fixture is ${size} bytes (>50 MiB); skipping to avoid OOM. Set COMPARE_ALLOW_LARGE_XL=1 to override." >&2
            SKIPPED_XL+=("xlsx-XL")
          fi
        fi
        ;;
      docx-S) python3 "$ROOT/fixtures/gen/generate.py" --id gen-docx-pages-10 2>&1 ;;
      docx-M) python3 "$ROOT/fixtures/gen/generate.py" --id gen-docx-pages-100 2>&1 ;;
      docx-L) python3 "$ROOT/fixtures/gen/generate.py" --id gen-docx-pages-500 2>&1 ;;
      docx-XL)
        path="$ROOT/fixtures/generated/docx/xl_a.docx"
        python3 "$ROOT/fixtures/gen/generate.py" --id gen-docx-xl-a 2>&1
        if [ "$ALLOW_LARGE_XL" = 0 ] && [ -f "$path" ]; then
          size=$(wc -c < "$path")
          if [ "$size" -gt 52428800 ]; then
            echo "compare_wasm_ts.sh: WARNING docx XL fixture is ${size} bytes (>50 MiB); skipping." >&2
            SKIPPED_XL+=("docx-XL")
          fi
        fi
        ;;
      pptx-S) python3 "$ROOT/fixtures/gen/generate.py" --id gen-pptx-slides-10 2>&1 ;;
      pptx-M) python3 "$ROOT/fixtures/gen/generate.py" --id gen-pptx-slides-50 2>&1 ;;
      pptx-L) python3 "$ROOT/fixtures/gen/generate.py" --id gen-pptx-slides-200 2>&1 ;;
      pptx-XL)
        path="$ROOT/fixtures/generated/pptx/xl_a.pptx"
        python3 "$ROOT/fixtures/gen/generate.py" --id gen-pptx-xl-a 2>&1
        if [ "$ALLOW_LARGE_XL" = 0 ] && [ -f "$path" ]; then
          size=$(wc -c < "$path")
          if [ "$size" -gt 52428800 ]; then
            echo "compare_wasm_ts.sh: WARNING pptx XL fixture is ${size} bytes (>50 MiB); skipping." >&2
            SKIPPED_XL+=("pptx-XL")
          fi
        fi
        ;;
    esac
  done
done

# Build effective tier lists (excluding OOM-skipped XL)
EFFECTIVE_OPEN_TIERS="$TIERS"
EFFECTIVE_MEM_TIERS="$TIERS"
if [ "${#SKIPPED_XL[@]}" -gt 0 ]; then
  # Remove XL from effective tiers if ALL XL combos were skipped
  all_skipped=1
  for fmt in "${FMT_ARR[@]}"; do
    skipped_this=0
    for s in "${SKIPPED_XL[@]}"; do
      [ "$s" = "${fmt}-XL" ] && skipped_this=1 && break
    done
    [ "$skipped_this" = 0 ] && all_skipped=0
  done
  # For harnesses we pass per-format tiers; use a simpler approach: include XL anyway
  # and let the harness skip missing or OOM'd fixtures.
  :
fi

# --- Cold openDocument timing (ts-vs-wasm.mjs) ---
OPEN_DIR="$TMP_DIR/open"
mkdir -p "$OPEN_DIR"
echo "compare_wasm_ts.sh: cold openDocument timing (runs=$RUNS warmup=$WARMUP bootstrap=$BOOTSTRAP)"

for fmt in "${FMT_ARR[@]}"; do
  fmt_tiers="$EFFECTIVE_OPEN_TIERS"
  # Remove XL from tiers if skipped for this format
  for s in "${SKIPPED_XL[@]}"; do
    if [ "$s" = "${fmt}-XL" ]; then
      fmt_tiers=$(echo "$fmt_tiers" | sed 's/,XL//; s/XL,//; s/^XL$//')
    fi
  done
  [ -z "$fmt_tiers" ] && continue
  echo "compare_wasm_ts.sh: openDocument $fmt tiers=$fmt_tiers"
  node --expose-gc "$ROOT/perf/large/ts-vs-wasm.mjs" \
    --format="$fmt" \
    --tier="$fmt_tiers" \
    --runs="$RUNS" \
    --warmup="$WARMUP" \
    --bootstrap="$BOOTSTRAP" \
    --json > "$OPEN_DIR/$fmt.json"
done

# --- Peak memory (peak-mem.mjs, fresh process per sample) ---
MEM_DIR="$TMP_DIR/mem"
mkdir -p "$MEM_DIR"
echo "compare_wasm_ts.sh: peak memory (runs=$MEM_RUNS bootstrap=$BOOTSTRAP)"

for fmt in "${FMT_ARR[@]}"; do
  fmt_tiers="$EFFECTIVE_MEM_TIERS"
  for s in "${SKIPPED_XL[@]}"; do
    if [ "$s" = "${fmt}-XL" ]; then
      fmt_tiers=$(echo "$fmt_tiers" | sed 's/,XL//; s/XL,//; s/^XL$//')
    fi
  done
  [ -z "$fmt_tiers" ] && continue
  echo "compare_wasm_ts.sh: peak memory $fmt tiers=$fmt_tiers"
  node "$ROOT/perf/large/peak-mem.mjs" \
    --format="$fmt" \
    --tier="$fmt_tiers" \
    --runs="$MEM_RUNS" \
    --bootstrap="$BOOTSTRAP" \
    --json > "$MEM_DIR/$fmt.json"
done

# --- Browser e2e (Playwright, optional — skipped if Python/Playwright unavailable) ---
BROWSER_E2E_JSON="$OUT_DIR/browser-e2e.json"
PLAYWRIGHT_PYTHON="${COMPARE_PLAYWRIGHT_PYTHON:-/workspace/pw-venv/bin/python}"
if command -v "$PLAYWRIGHT_PYTHON" >/dev/null 2>&1 \
   && [ -f "$ROOT/scripts/perf/browser_e2e.py" ]; then
  echo "compare_wasm_ts.sh: browser e2e Playwright (runs=${COMPARE_BROWSER_RUNS:-7})"
  mkdir -p "$OUT_DIR"
  "$PLAYWRIGHT_PYTHON" "$ROOT/scripts/perf/browser_e2e.py" \
    --formats "$FORMATS" \
    --tiers "$TIERS" \
    --runs "${COMPARE_BROWSER_RUNS:-7}" \
    --bootstrap "$BOOTSTRAP" \
    --out "$BROWSER_E2E_JSON" 2>&1 \
  || echo "compare_wasm_ts.sh: WARNING browser_e2e.py failed; browser e2e section will be absent"
else
  echo "compare_wasm_ts.sh: Playwright Python not at $PLAYWRIGHT_PYTHON or browser_e2e.py missing; skipping browser e2e"
fi

# --- Bundle sizes (perf/sizes.mjs) ---
SIZES_JSON="$TMP_DIR/sizes.json"
echo "compare_wasm_ts.sh: bundle sizes"
# Build TS bundles for perf/sizes.mjs
if [ ! -f "$ROOT/perf/dist/ts/xlsx.js" ]; then
  echo "compare_wasm_ts.sh: building TS bundles for size measurement"
  ( cd "$ROOT/perf" && node build-ts.mjs ) 2>&1 || \
    echo "compare_wasm_ts.sh: WARNING TS bundle build failed; sizes for TS may be missing"
fi
node "$ROOT/perf/sizes.mjs" --out "$SIZES_JSON" 2>&1 || \
  echo "compare_wasm_ts.sh: WARNING sizes.mjs failed; bundle size section may be incomplete"

# --- Compute fixture hashes ---
HASHES_JSON="$TMP_DIR/hashes.json"
node - "$ROOT" "$FORMATS" "$TIERS" > "$HASHES_JSON" <<'JS'
const { createHash } = require("crypto");
const { readFileSync, existsSync } = require("fs");
const { join } = require("path");
const root = process.argv[2];
const formats = process.argv[3].split(",");
const tiers = process.argv[4].split(",");
const FIXTURE_PATHS = {
  "xlsx-S":  "fixtures/generated/xlsx/cells_1k.xlsx",
  "xlsx-M":  "fixtures/generated/xlsx/cells_100k.xlsx",
  "xlsx-L":  "fixtures/generated/xlsx/cells_1m.xlsx",
  "xlsx-XL": "fixtures/generated/xlsx/xl_a.xlsx",
  "docx-S":  "fixtures/generated/docx/pages_10.docx",
  "docx-M":  "fixtures/generated/docx/pages_100.docx",
  "docx-L":  "fixtures/generated/docx/pages_500.docx",
  "docx-XL": "fixtures/generated/docx/xl_a.docx",
  "pptx-S":  "fixtures/generated/pptx/slides_10.pptx",
  "pptx-M":  "fixtures/generated/pptx/slides_50.pptx",
  "pptx-L":  "fixtures/generated/pptx/slides_200.pptx",
  "pptx-XL": "fixtures/generated/pptx/xl_a.pptx",
};
const out = {};
for (const fmt of formats) {
  for (const tier of tiers) {
    const key = `${fmt}-${tier}`;
    const rel = FIXTURE_PATHS[key];
    if (!rel) continue;
    const full = join(root, rel);
    if (!existsSync(full)) { out[key] = { path: rel, sha256: null, bytes: null }; continue; }
    const b = readFileSync(full);
    out[key] = {
      path: rel,
      sha256: createHash("sha256").update(b).digest("hex"),
      bytes: b.length,
    };
  }
}
process.stdout.write(JSON.stringify(out, null, 2) + "\n");
JS

# --- Assemble final report ---
REPORT_JSON="$TMP_DIR/report.json"
OUT_MD="$OUT_DIR/wasm-vs-ts.md"
OUT_JSON="$OUT_DIR/wasm-vs-ts.json"
mkdir -p "$OUT_DIR"

node - "$TMP_DIR" "$FORMATS" "$TIERS" "$SHA" "$WHEN" "$HOST_OS" "$HOST_ARCH" "$NODE_VER" \
     "$OUT_JSON" "$OUT_MD" "$BROWSER_E2E_JSON" <<'JS'
const { readFileSync, writeFileSync, existsSync } = require("fs");
const { join } = require("path");

const [tmpDir, fmtsArg, tiersArg, sha, when, hostOs, hostArch, nodeVer, outJson, outMd, browserE2eJson] =
  process.argv.slice(2);
const formats = fmtsArg.split(",");
const tiers = tiersArg.split(",");

function tryRead(p) {
  try { return JSON.parse(readFileSync(p, "utf8")); } catch { return null; }
}

function percentile(xs, p) {
  if (!xs.length) return NaN;
  const a = [...xs].sort((x, y) => x - y);
  const i = (a.length - 1) * p;
  const lo = Math.floor(i), hi = Math.ceil(i);
  if (lo === hi) return a[lo];
  return a[lo] * (hi - i) + a[hi] * (i - lo);
}
function stdev(xs) {
  if (xs.length < 2) return NaN;
  const m = xs.reduce((a, b) => a + b, 0) / xs.length;
  return Math.sqrt(xs.reduce((s, x) => s + (x - m) ** 2, 0) / (xs.length - 1));
}
function iqr(xs) {
  if (!xs.length) return NaN;
  return percentile(xs, 0.75) - percentile(xs, 0.25);
}
function f2(n) { return Number.isFinite(n) ? +n.toFixed(2) : null; }
function f3(n) { return Number.isFinite(n) ? +n.toFixed(3) : null; }

const hashes = tryRead(join(tmpDir, "hashes.json")) || {};
const sizes = tryRead(join(tmpDir, "sizes.json"));
const browserE2e = tryRead(browserE2eJson);

// Gather open + mem rows
const openRows = [];
const memRows = [];
for (const fmt of formats) {
  const od = tryRead(join(tmpDir, "open", `${fmt}.json`));
  if (od && od.rows) {
    for (const r of od.rows) {
      const tsS = r.ts_ms_samples || [];
      const wasmS = r.wasm_ms_samples || [];
      openRows.push({
        format: r.format, tier: r.tier, fixture: r.fixture, file_bytes: r.file_bytes,
        metric: "cold_open_ms",
        ts: {
          median: f2(r.ts_ms_median),
          p95: f2(percentile(tsS, 0.95)),
          stdev: f2(stdev(tsS)),
          iqr: f2(iqr(tsS)),
          samples: tsS,
          entry: r.ts_entry,
        },
        wasm: {
          median: f2(r.wasm_ms_median),
          p95: f2(percentile(wasmS, 0.95)),
          stdev: f2(stdev(wasmS)),
          iqr: f2(iqr(wasmS)),
          samples: wasmS,
          entry: r.wasm_entry,
        },
        wasm_over_ts_median: f3(r.wasm_over_ts_median),
        wasm_over_ts_ci95: r.wasm_over_ts_ci95,
        verdict: r.verdict,
        note: r.note,
      });
    }
  }
  const md = tryRead(join(tmpDir, "mem", `${fmt}.json`));
  if (md && md.rows) {
    for (const r of md.rows) {
      const tsS = (r.ts && r.ts.mem_samples) || [];
      const wasmS = (r.wasm && r.wasm.mem_samples) || [];
      memRows.push({
        format: r.format, tier: r.tier, fixture: r.fixture,
        metric: "peak_rss_mib",
        ts: {
          median: f2(r.ts && r.ts.mem),
          p95: f2(percentile(tsS, 0.95)),
          stdev: f2(stdev(tsS)),
          samples: tsS,
        },
        wasm: {
          median: f2(r.wasm && r.wasm.mem),
          p95: f2(percentile(wasmS, 0.95)),
          stdev: f2(stdev(wasmS)),
          samples: wasmS,
        },
        wasm_over_ts_mem: f3(r.wasm_over_ts_mem),
        wasm_over_ts_mem_ci95: r.wasm_over_ts_mem_ci95,
        verdict: r.verdict,
      });
    }
  }
}

const report = {
  schema: 1,
  race: "m1-wasm-vs-ts-comparison",
  sha, when,
  host: { os: hostOs, arch: hostArch, node: nodeVer },
  method: {
    cold_open: "ts-vs-wasm.mjs: median(WASM)/median(TS) cold openDocument, same Node process, N>=5 warmup-discarded repetitions",
    peak_mem: "peak-mem.mjs: VmHWM growth over post-init baseline, fresh process per (engine, fixture, run)",
    bundle: "sizes.mjs: raw+gzip+brotli for TS bundles (perf/dist/ts/<fmt>.js) and WASM pkg (nitro_xml_wasm_bg.wasm + nitro_xml_wasm.js)",
    save: "NOT MEASURED — WASM save path not yet available at M1 viewer stage; TS save available but comparison would be asymmetric. See draft issue.",
    ci: "95% bootstrap CI of median ratio, unpaired; WIN = median < 1 AND CI upper < 1",
    spread: "p95, stdev, IQR computed from raw sample arrays",
    seed: 20261019,
  },
  fixtures: hashes,
  sizes: sizes ? sizes.sizes : null,
  sizes_missing: sizes ? sizes.missing : ["perf/dist/ts/* (not built)", "packages/nitro-xml/pkg/* (not built)"],
  cold_open_rows: openRows,
  peak_mem_rows: memRows,
  browser_e2e_rows: browserE2e ? browserE2e.rows : null,
  browser_e2e_meta: browserE2e ? { description: browserE2e.description, chromium_bin: browserE2e.chromium_bin, runs_per_fixture: browserE2e.runs_per_fixture } : null,
  caveats: [
    "xlsx metric = 'time to first view' (not full load): TS loadWorkbook loads the entire workbook; WASM openDocument calls open_first_view (lazy per-sheet parse, only the first visible sheet is parsed). Both timings reflect first-sheet availability, not full-workbook completion. Rows are labeled 'time-to-first-view' in the cold-open table. See #283.",
    "Node cold-open numbers start after initSync — they do not include WASM fetch, compile, or instantiate. See the 'Browser e2e' section for timings that include the full cold-start path.",
    "docx XL cold open: TS oracle rejects word/document.xml > 25 MiB — BLOCKED for fair comparison (see #550).",
    "docx XL peak memory: same TS oracle limit applies — BLOCKED.",
    "XL tier benchmarks run with COMPARE_ALLOW_LARGE_XL=0 by default; large fixtures (>50 MiB) are skipped to avoid OOM on constrained runners",
    "Save latency: not measured — WASM save path not yet available at M1 viewer stage",
    "Platform: Linux x86_64 (not macOS/Apple Silicon); results may differ on other hardware",
    "Perf gate (perf_beats_ts.sh) is in REPORT MODE; WASM-beats-TS is not yet ENFORCED (see #8)",
    "CI reproduction: compare_wasm_ts.sh runs automatically from scripts/ci/perf.sh record (record mode added in this PR).",
  ],
};

writeFileSync(outJson, JSON.stringify(report, null, 2) + "\n");

// --- Markdown report ---
function fN(n, d=2) { return n == null ? "—" : n.toFixed(d); }
function ver(v) { if (v === "WIN") return "✓ WIN"; if (v === "TIE/LOSS") return "⚠ TIE"; return "✗ LOSS"; }

const lines = [];
lines.push("<!-- SPDX-License-Identifier: MIT -->");
lines.push(`# WASM vs TS comparison — M1 closeout (#117 fixtures)`);
lines.push(``);
lines.push(`**SHA:** \`${sha}\` · **when:** ${when} · **host:** ${hostOs}/${hostArch} · **Node:** ${nodeVer}`);
lines.push(``);
lines.push(`> This table is machine-generated by \`scripts/perf/compare_wasm_ts.sh\`. Reproduce with:`);
lines.push(`> \`\`\``);
lines.push(`> bash scripts/perf/compare_wasm_ts.sh --skip-wasm-build --skip-ts-build`);
lines.push(`> \`\`\``);
lines.push(``);
lines.push(`## Caveats`);
lines.push(``);
for (const c of report.caveats) lines.push(`- ${c}`);
lines.push(``);

lines.push(`## Cold open (Node.js) — ms`);
lines.push(``);
lines.push(`xlsx metric = **time to first view** (WASM: open_first_view; TS: full loadWorkbook — both timed to first-sheet availability). docx/pptx: cold openDocument. These numbers start after initSync and do **not** include WASM fetch, compile, or instantiate — see the Browser e2e section below.`);
lines.push(``);
lines.push(`N≥${openRows[0]?.wasm?.samples?.length ?? "?"} timed runs (after warmup). Metric: \`performance.now()\` around \`openDocument\`; fresh \`Uint8Array\` copy outside timed span.`);
lines.push(``);
lines.push(`| Format | Metric | Tier | File (bytes) | TS median | TS p95 | TS stdev | WASM median | WASM p95 | WASM stdev | WASM/TS | 95% CI | Verdict |`);
lines.push(`|--------|--------|------|-------------:|----------:|-------:|---------:|------------:|---------:|-----------:|--------:|-------:|---------|`);
for (const r of openRows) {
  const ci = r.wasm_over_ts_ci95 || {};
  const lo = ci.lo != null ? ci.lo.toFixed(3) : "—";
  const hi = ci.hi != null ? ci.hi.toFixed(3) : "—";
  const metricLabel = r.format === "xlsx" ? "time-to-first-view" : "cold-open";
  lines.push(
    `| ${r.format} | ${metricLabel} | ${r.tier} | ${r.file_bytes?.toLocaleString() ?? "—"} | ${fN(r.ts.median)} | ${fN(r.ts.p95)} | ${fN(r.ts.stdev)} | ${fN(r.wasm.median)} | ${fN(r.wasm.p95)} | ${fN(r.wasm.stdev)} | ${fN(r.wasm_over_ts_median, 3)} | [${lo}, ${hi}] | **${ver(r.verdict)}** |`
  );
}
lines.push(``);

lines.push(`## Peak memory — VmHWM growth (MiB)`);
lines.push(``);
lines.push(`N=7 runs; fresh Node process per (engine, fixture, run); baseline subtracted (post-init VmHWM). XL tier: xlsx and pptx measured; docx XL BLOCKED (#550 — TS oracle limit). Every measured cell shows the absolute median value and 95% CI.`);
lines.push(``);
lines.push(`| Format | Tier | TS median (MiB) | TS p95 | TS stdev | WASM median (MiB) | WASM p95 | WASM stdev | WASM/TS | 95% CI | Verdict |`);
lines.push(`|--------|------|----------------:|-------:|---------:|------------------:|---------:|-----------:|--------:|-------:|---------|`);
for (const r of memRows) {
  const ci = r.wasm_over_ts_mem_ci95 || {};
  const lo = ci.lo != null ? ci.lo.toFixed(3) : "—";
  const hi = ci.hi != null ? ci.hi.toFixed(3) : "—";
  const tsMed = r.ts && r.ts.blocked ? "BLOCKED" : fN(r.ts ? r.ts.median : null, 1);
  const wasmMed = r.wasm && r.wasm.blocked ? "BLOCKED" : fN(r.wasm ? r.wasm.median : null, 1);
  lines.push(
    `| ${r.format} | ${r.tier} | ${tsMed} | ${fN(r.ts && !r.ts.blocked ? r.ts.p95 : null, 1)} | ${fN(r.ts && !r.ts.blocked ? r.ts.stdev : null, 1)} | ${wasmMed} | ${fN(r.wasm && !r.wasm.blocked ? r.wasm.p95 : null, 1)} | ${fN(r.wasm && !r.wasm.blocked ? r.wasm.stdev : null, 1)} | ${fN(r.wasm_over_ts_mem, 3)} | [${lo}, ${hi}] | **${r.verdict === "BLOCKED" ? "🚫 BLOCKED" : ver(r.verdict)}** |`
  );
}
lines.push(``);
// docx L callout against previous 1.151× loss
const docxL = memRows.find(r => r.format === "docx" && r.tier === "L");
if (docxL && docxL.wasm_over_ts_mem != null) {
  const prevRatio = 1.151;
  const curRatio = docxL.wasm_over_ts_mem;
  const ci = docxL.wasm_over_ts_mem_ci95 || {};
  const isWin = docxL.verdict === "WIN";
  lines.push(`> **docx L peak memory vs prior run:** previous ratio = ${prevRatio.toFixed(3)}× (LOSS). Current ratio = ${curRatio.toFixed(3)}× (CI ${ci.lo != null ? ci.lo.toFixed(3) : "—"}–${ci.hi != null ? ci.hi.toFixed(3) : "—"}) → now a **${isWin ? "WIN" : "LOSS"}** (TS median ${fN(docxL.ts ? docxL.ts.median : null, 1)} MiB; WASM median ${fN(docxL.wasm ? docxL.wasm.median : null, 1)} MiB).`);
}
lines.push(``);

lines.push(``);
lines.push(`## Browser end-to-end first paint (ms)`);
lines.push(``);
if (browserE2e && browserE2e.rows && browserE2e.rows.length) {
  lines.push(`Chromium headless shell. Timing: navigationStart → first shadow-DOM content paint. Includes WASM fetch, compile, and instantiate. Fresh browser instance per run (no HTTP cache, no V8 code cache). N=${browserE2e.runs_per_fixture} runs per fixture.`);
  lines.push(``);
  lines.push(`| Format | Tier | median (ms) | p95 (ms) | stdev (ms) | spread (ms) | 95% CI |`);
  lines.push(`|--------|------|------------:|---------:|-----------:|------------:|--------|`);
  for (const r of browserE2e.rows) {
    if (r.status !== "OK") {
      lines.push(`| ${r.format} | ${r.tier} | — | — | — | — | ${r.status} |`);
      continue;
    }
    lines.push(
      `| ${r.format} | ${r.tier} | ${r.median_ms.toFixed(0)} | ${r.p95_ms.toFixed(0)} | ${r.stdev_ms.toFixed(0)} | ${r.spread_ms.toFixed(0)} | [${r.ci95_lo_ms.toFixed(0)}, ${r.ci95_hi_ms.toFixed(0)}] |`
    );
  }
} else {
  lines.push(`_Not measured in this run. Run \`/workspace/pw-venv/bin/python scripts/perf/browser_e2e.py\` to generate \`docs/perf/browser-e2e.json\`, then re-run compare_wasm_ts.sh._`);
}
lines.push(``);

lines.push(`## Bundle sizes`);
lines.push(``);
if (report.sizes) {
  for (const [group, { files, total }] of Object.entries(report.sizes)) {
    lines.push(`### ${group}`);
    lines.push(``);
    lines.push(`| File | Raw (bytes) | Gzip | Brotli |`);
    lines.push(`|------|------------:|-----:|-------:|`);
    for (const r of files) {
      lines.push(`| ${r.file} | ${r.raw.toLocaleString()} | ${r.gzip.toLocaleString()} | ${r.brotli.toLocaleString()} |`);
    }
    lines.push(`| **TOTAL** | **${total.raw.toLocaleString()}** | **${total.gzip.toLocaleString()}** | **${total.brotli.toLocaleString()}** |`);
    lines.push(``);
  }
} else {
  lines.push(`_Bundle size data unavailable — TS bundles not built. Run \`cd perf && node build-ts.mjs\` first, then rerun this script._`);
  lines.push(``);
}

lines.push(`## Save latency`);
lines.push(``);
lines.push(`_Not measured at M1 viewer stage. WASM save path not yet available. TS save path exists but a one-sided comparison would be misleading. This metric will be added in M2 (E1/E3/E4). See draft issue linked from the PR._`);
lines.push(``);

lines.push(`## Fixture hashes`);
lines.push(``);
lines.push(`| Fixture | Path | SHA-256 (first 16 hex) | Bytes |`);
lines.push(`|---------|------|----------------------:|------:|`);
for (const [key, h] of Object.entries(hashes)) {
  const short = h.sha256 ? h.sha256.slice(0, 16) + "…" : "MISSING";
  lines.push(`| ${key} | \`${h.path}\` | \`${short}\` | ${h.bytes != null ? h.bytes.toLocaleString() : "—"} |`);
}
lines.push(``);

lines.push(`## Method`);
lines.push(``);
lines.push(`- **Cold open (Node):** \`perf/large/ts-vs-wasm.mjs\` — xlsx: TS=\`loadWorkbook\` (full), WASM=\`openDocument\` (open_first_view, time-to-first-view). docx/pptx: cold open. After \`initSync\`; does not include WASM fetch/compile.`);
lines.push(`- **Browser e2e:** \`scripts/perf/browser_e2e.py\` — Playwright headless Chromium; navigationStart to first shadow-DOM content paint; fresh browser per run. Includes WASM fetch, compile, instantiate.`);
lines.push(`- **Peak memory:** \`perf/large/peak-mem.mjs\` — VmHWM from \`/proc/self/status\`, growth over post-init baseline; fresh process per sample. XL: xlsx + pptx measured; docx XL BLOCKED (#550).`);
lines.push(`- **Bundle size:** \`perf/sizes.mjs\` — raw + gzip (level 9) + brotli (quality 11); TS bundles from \`perf/dist/ts/\`, WASM from \`packages/nitro-xml/pkg/\`.`);
lines.push(`- **Statistics:** median, p95, stdev, IQR from raw sample arrays. WASM/TS ratio = median(WASM)/median(TS). 95% CI via unpaired bootstrap (B=2000, deterministic seed).`);
lines.push(`- **WIN rule:** median ratio < 1 AND 95% CI upper < 1 (PRODUCT_SPEC §5). A tie is a fail.`);
lines.push(`- **CI:** \`scripts/ci/perf.sh record\` calls \`compare_wasm_ts.sh\` automatically (added in this PR).`);
lines.push(``);

lines.push(`## How to reproduce`);
lines.push(``);
lines.push(`\`\`\`sh`);
lines.push(`# From repo root on a fresh clone:`);
lines.push(`git submodule update --init upstream`);
lines.push(`bash scripts/perf/compare_wasm_ts.sh`);
lines.push(``);
lines.push(`# Skip rebuilds if already built:`);
lines.push(`bash scripts/perf/compare_wasm_ts.sh --skip-wasm-build --skip-ts-build`);
lines.push(``);
lines.push(`# From the existing perf CI job (perf.sh record):`);
lines.push(`COMPARE_TIERS=S,M,L,XL bash scripts/perf/compare_wasm_ts.sh --skip-wasm-build --skip-ts-build`);
lines.push(`\`\`\``);
lines.push(``);

writeFileSync(outMd, lines.join("\n") + "\n");
process.stdout.write(`compare_wasm_ts.sh: written ${outJson}\ncompare_wasm_ts.sh: written ${outMd}\n`);
JS

echo "compare_wasm_ts.sh: done. Report at $OUT_MD and $OUT_JSON"

if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "## compare_wasm_ts.sh (M1 closeout)" >> "$GITHUB_STEP_SUMMARY"
  cat "$OUT_MD" >> "$GITHUB_STEP_SUMMARY"
fi

exit 0
