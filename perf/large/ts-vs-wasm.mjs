#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Report-only TS vs WASM open timing for docs/M1-REPORT.md.
 *
 * Times the same fixtures with:
 *   - upstream ooxml-core (test oracle): loadWorkbook / loadDocument / PptxHandler.load
 *     (same entry points as perf/scoreboard adapters)
 *   - Nitro WASM: Handle.openDocument (viewer open path; xlsx uses open_first_view)
 *
 * Emits medians and a 95% bootstrap CI of the WASM/TS median ratio.
 * Does NOT arm gates; does not edit perf_beats_ts.sh / budgets / workflows.
 *
 * Usage (repo root):
 *   node perf/large/ts-vs-wasm.mjs [--runs=11] [--warmup=2] [--bootstrap=2000] [--json]
 *   node perf/large/ts-vs-wasm.mjs --format=xlsx --tier=L
 */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const PKG = join(ROOT, "packages/nitro-xml/pkg/nitro_xml_wasm.js");
const WASM = join(ROOT, "packages/nitro-xml/pkg/nitro_xml_wasm_bg.wasm");
const UP = join(ROOT, "upstream/src/core/dist");

const args = Object.fromEntries(
  process.argv.slice(2).map((a) => {
    const [k, v] = a.replace(/^--/, "").split("=");
    return [k, v ?? "true"];
  }),
);
const RUNS = Number(args.runs ?? 11);
const WARMUP = Number(args.warmup ?? 2);
const BOOTSTRAP = Number(args.bootstrap ?? 2000);
const SEED = Number(args.seed ?? 20261019);
const FORMAT_FILTER = args.format ? args.format.split(",") : null;
const TIER_FILTER = args.tier ? args.tier.split(",") : null;
const JSON_ONLY = args.json === "true";

const FIXTURES = [
  { format: "xlsx", tier: "S", rel: "fixtures/generated/xlsx/cells_1k.xlsx" },
  { format: "xlsx", tier: "M", rel: "fixtures/generated/xlsx/cells_100k.xlsx" },
  { format: "xlsx", tier: "L", rel: "fixtures/generated/xlsx/cells_1m.xlsx" },
  { format: "xlsx", tier: "XL", rel: "fixtures/generated/xlsx/xl_a.xlsx" },
  { format: "docx", tier: "S", rel: "fixtures/generated/docx/pages_10.docx" },
  { format: "docx", tier: "M", rel: "fixtures/generated/docx/pages_100.docx" },
  { format: "docx", tier: "L", rel: "fixtures/generated/docx/pages_500.docx" },
  { format: "docx", tier: "XL", rel: "fixtures/generated/docx/xl_a.docx" },
  { format: "pptx", tier: "S", rel: "fixtures/generated/pptx/slides_10.pptx" },
  { format: "pptx", tier: "M", rel: "fixtures/generated/pptx/slides_50.pptx" },
  { format: "pptx", tier: "L", rel: "fixtures/generated/pptx/slides_200.pptx" },
  { format: "pptx", tier: "XL", rel: "fixtures/generated/pptx/xl_a.pptx" },
];

function median(xs) {
  if (!xs.length) return NaN;
  const a = [...xs].sort((x, y) => x - y);
  const m = Math.floor(a.length / 2);
  return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2;
}

function percentile(xs, p) {
  if (!xs.length) return NaN;
  const a = [...xs].sort((x, y) => x - y);
  const i = (a.length - 1) * p;
  const lo = Math.floor(i);
  const hi = Math.ceil(i);
  if (lo === hi) return a[lo];
  return a[lo] * (hi - i) + a[hi] * (i - lo);
}

/** Mulberry32 — deterministic bootstrap draws. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * 95% bootstrap CI of median(wasm) / median(ts).
 * Unpaired: resample each engine's samples independently, take median ratio.
 */
function bootstrapRatioCi(wasmSamples, tsSamples, B, seed) {
  const rand = rng(seed);
  const ratios = new Array(B);
  for (let b = 0; b < B; b++) {
    const w = [];
    const t = [];
    for (let i = 0; i < wasmSamples.length; i++) {
      w.push(wasmSamples[Math.floor(rand() * wasmSamples.length)]);
    }
    for (let i = 0; i < tsSamples.length; i++) {
      t.push(tsSamples[Math.floor(rand() * tsSamples.length)]);
    }
    const tm = median(t);
    ratios[b] = tm > 0 ? median(w) / tm : NaN;
  }
  const finite = ratios.filter((x) => Number.isFinite(x));
  return {
    lo: percentile(finite, 0.025),
    hi: percentile(finite, 0.975),
    B: finite.length,
  };
}

function gitSha() {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], { cwd: ROOT, encoding: "utf8" }).trim();
  } catch {
    return "unknown";
  }
}

function copyBytes(file) {
  // Fresh copy per run (outside timed span in callers).
  return new Uint8Array(file);
}

async function loadTsOpeners() {
  const { loadWorkbook } = await import(pathToFileURL(join(UP, "xlsx/load/index.mjs")).href);
  const { loadDocument } = await import(pathToFileURL(join(UP, "docx/load/index.mjs")).href);
  const { PptxHandler } = await import(pathToFileURL(join(UP, "pptx/index.mjs")).href);
  return {
    xlsx: async (bytes) => {
      await loadWorkbook(bytes);
    },
    docx: async (bytes) => {
      await loadDocument(bytes);
    },
    pptx: async (bytes) => {
      const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
      await new PptxHandler().load(buf);
    },
  };
}

async function loadWasmOpener() {
  if (!existsSync(PKG) || !existsSync(WASM)) {
    throw new Error(`missing wasm pkg at ${PKG}; run bash scripts/build-wasm.sh`);
  }
  const { initSync, Handle } = await import(pathToFileURL(PKG).href);
  initSync({ module: readFileSync(WASM) });
  return (bytes, name) => {
    const h = new Handle(copyBytes(bytes));
    h.openDocument(name);
    h.free();
  };
}

async function timeEngine(label, fn, runs, warmup) {
  const samples = [];
  for (let i = 0; i < warmup + runs; i++) {
    const t0 = performance.now();
    await fn();
    const elapsed = performance.now() - t0;
    if (i >= warmup) samples.push(elapsed);
  }
  return samples;
}

function verdict(ratioMed, ci) {
  if (!(ratioMed < 1) || !(ci.hi < 1)) {
    if (ratioMed > 1 || ci.lo > 1) return "LOSS";
    if (Math.abs(ratioMed - 1) < 1e-9 || (ci.lo <= 1 && ci.hi >= 1 && !(ci.hi < 1))) return "TIE/LOSS";
    return "LOSS";
  }
  return "WIN";
}

function fmt(n, digits = 2) {
  if (!Number.isFinite(n)) return "—";
  return n.toFixed(digits);
}

async function main() {
  const sha = gitSha();
  const when = new Date().toISOString();
  const host = `${process.platform} ${process.arch} node ${process.version}`;
  const tsOpen = await loadTsOpeners();
  const wasmOpen = await loadWasmOpener();

  const rows = [];
  const selected = FIXTURES.filter((f) => {
    if (FORMAT_FILTER && !FORMAT_FILTER.includes(f.format)) return false;
    if (TIER_FILTER && !TIER_FILTER.includes(f.tier)) return false;
    return true;
  });

  for (const f of selected) {
    const path = join(ROOT, f.rel);
    if (!existsSync(path)) {
      console.error(`missing fixture ${f.rel}; generate with fixtures/gen/generate.py`);
      process.exit(1);
    }
    const file = readFileSync(path);
    const name = path.split("/").pop();
    if (!JSON_ONLY) {
      console.error(`timing ${f.format} ${f.tier} ${f.rel} (${file.byteLength} B) …`);
    }

    const tsSamples = await timeEngine(
      "ts",
      async () => {
        await tsOpen[f.format](copyBytes(file));
      },
      RUNS,
      WARMUP,
    );
    // GC hint between engines
    if (typeof globalThis.gc === "function") globalThis.gc();

    const wasmSamples = await timeEngine(
      "wasm",
      async () => {
        wasmOpen(file, name);
      },
      RUNS,
      WARMUP,
    );

    const tsMed = median(tsSamples);
    const wasmMed = median(wasmSamples);
    const ratio = wasmMed / tsMed;
    const ci = bootstrapRatioCi(wasmSamples, tsSamples, BOOTSTRAP, SEED + rows.length * 97);
    const v = verdict(ratio, ci);

    rows.push({
      format: f.format,
      tier: f.tier,
      fixture: f.rel,
      file_bytes: file.byteLength,
      metric: "cold_open_ms",
      ts_entry:
        f.format === "xlsx"
          ? "ooxml-core/xlsx/load loadWorkbook"
          : f.format === "docx"
            ? "ooxml-core/docx/load loadDocument"
            : "ooxml-core/pptx PptxHandler.load",
      wasm_entry: "Handle.openDocument",
      runs: RUNS,
      warmup: WARMUP,
      ts_ms_median: tsMed,
      ts_ms_samples: tsSamples.map((x) => +x.toFixed(3)),
      wasm_ms_median: wasmMed,
      wasm_ms_samples: wasmSamples.map((x) => +x.toFixed(3)),
      wasm_over_ts_median: ratio,
      wasm_over_ts_ci95: { lo: ci.lo, hi: ci.hi, bootstrap: ci.B },
      verdict: v,
      note:
        f.format === "xlsx"
          ? "TS loadWorkbook is a full workbook parse; WASM openDocument uses open_first_view (viewport) + first process_frame — not identical work, especially at L/XL after #283."
          : null,
    });
  }

  const report = {
    schema: 1,
    race: "m1-report-ts-vs-wasm-cold-open",
    sha,
    when,
    host,
    method: {
      warmupRuns: WARMUP,
      timedRuns: RUNS,
      bootstrap: BOOTSTRAP,
      seed: SEED,
      timer: "performance.now(); fresh Uint8Array copy outside timed span",
      ts: "upstream ooxml-core dist (submodule), same entry points as perf/scoreboard adapters",
      wasm: "packages/nitro-xml/pkg Handle.openDocument after initSync (Node; not browser first-paint)",
      ratio: "median(wasm)/median(ts); 95% CI via unpaired bootstrap of that median ratio",
      beats_rule:
        "WIN = median ratio < 1 AND CI upper < 1 (PRODUCT_SPEC §5); otherwise LOSS or TIE/LOSS — reported honestly, gate not armed",
    },
    rows,
  };

  const outDir = join("/tmp", `m1-ts-vs-wasm-${sha.slice(0, 7)}`);
  mkdirSync(outDir, { recursive: true });
  const outJson = join(outDir, "ts-vs-wasm.json");
  writeFileSync(outJson, JSON.stringify(report, null, 2));

  if (JSON_ONLY) {
    process.stdout.write(JSON.stringify(report, null, 2) + "\n");
    return;
  }

  console.log(`# TS vs WASM cold open — sha ${sha}`);
  console.log(`when=${when}`);
  console.log(`host=${host}`);
  console.log(`runs=${RUNS} warmup=${WARMUP} bootstrap=${BOOTSTRAP} seed=${SEED}`);
  console.log(`json=${outJson}`);
  console.log("");
  console.log(
    "| Format | Tier | TS median (ms) | WASM median (ms) | WASM/TS | 95% CI | Verdict |",
  );
  console.log("|--------|------|---------------:|-----------------:|--------:|-------:|---------|");
  for (const r of rows) {
    console.log(
      `| ${r.format} | ${r.tier} | ${fmt(r.ts_ms_median)} | ${fmt(r.wasm_ms_median)} | ${fmt(r.wasm_over_ts_median, 3)} | [${fmt(r.wasm_over_ts_ci95.lo, 3)}, ${fmt(r.wasm_over_ts_ci95.hi, 3)}] | **${r.verdict}** |`,
    );
  }
  console.log("");
  console.log("Metric: cold open only (not scroll / peak memory / browser first-paint).");
  console.log("xlsx asymmetry: TS=full loadWorkbook; WASM=open_first_view + process_frame.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
