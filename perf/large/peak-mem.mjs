#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Peak-memory + cold-open table for TS (upstream ooxml-core) vs Nitro WASM, one fresh process per
 * (engine, fixture, run) so VmHWM (process-lifetime peak RSS) is attributable to a single open.
 *
 *   node perf/large/peak-mem.mjs [--runs=5] [--pkg=<dir with nitro_xml_wasm.js + _bg.wasm>]
 *        [--tier=S,M,L] [--format=xlsx,docx,pptx] [--bootstrap=2000] [--seed=20261019] [--json]
 *
 * Reports, per fixture: median open ms and median peak-RSS growth (MiB) over the post-init
 * baseline for each engine, plus the WASM/TS ratios and a 95% bootstrap CI of the peak-mem
 * median ratio (same unpaired method as ts-vs-wasm.mjs). Child mode: `--child=<engine>:<fixture>`.
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const args = Object.fromEntries(
  process.argv.slice(2).map((a) => {
    const [k, v] = a.replace(/^--/, "").split("=");
    return [k, v ?? "true"];
  }),
);
const PKG_DIR = args.pkg ?? join(ROOT, "packages/nitro-xml/pkg");
const UP = join(ROOT, "upstream/src/core/dist");
const BOOTSTRAP = Number(args.bootstrap ?? 2000);
const SEED = Number(args.seed ?? 20261019);
const FIXTURES = [
  ["xlsx", "S", "fixtures/generated/xlsx/cells_1k.xlsx"],
  ["xlsx", "M", "fixtures/generated/xlsx/cells_100k.xlsx"],
  ["xlsx", "L", "fixtures/generated/xlsx/cells_1m.xlsx"],
  ["docx", "S", "fixtures/generated/docx/pages_10.docx"],
  ["docx", "M", "fixtures/generated/docx/pages_100.docx"],
  ["docx", "L", "fixtures/generated/docx/pages_500.docx"],
  ["pptx", "S", "fixtures/generated/pptx/slides_10.pptx"],
  ["pptx", "M", "fixtures/generated/pptx/slides_50.pptx"],
  ["pptx", "L", "fixtures/generated/pptx/slides_200.pptx"],
];

function vmHwmMiB() {
  const m = /VmHWM:\s+(\d+) kB/.exec(readFileSync("/proc/self/status", "utf8"));
  return m ? Number(m[1]) / 1024 : NaN;
}

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

/** Mulberry32 — deterministic bootstrap draws (same as ts-vs-wasm.mjs). */
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

/** WIN = median ratio < 1 AND CI upper < 1 (PRODUCT_SPEC §5); else LOSS / TIE/LOSS. */
function verdict(ratioMed, ci) {
  if (!(ratioMed < 1) || !(ci.hi < 1)) {
    if (ratioMed > 1 || ci.lo > 1) return "LOSS";
    if (Math.abs(ratioMed - 1) < 1e-9 || (ci.lo <= 1 && ci.hi >= 1 && !(ci.hi < 1))) return "TIE/LOSS";
    return "LOSS";
  }
  return "WIN";
}

async function child(spec) {
  const [engine, rel] = spec.split(":");
  const file = readFileSync(join(ROOT, rel));
  const name = rel.split("/").pop();
  const fmt = name.split(".").pop();
  let open;
  if (engine === "ts") {
    const imp = (p) => import(pathToFileURL(join(UP, p)).href);
    if (fmt === "xlsx") {
      const { loadWorkbook } = await imp("xlsx/load/index.mjs");
      open = (b) => loadWorkbook(b);
    } else if (fmt === "docx") {
      const { loadDocument } = await imp("docx/load/index.mjs");
      open = (b) => loadDocument(b);
    } else {
      const { PptxHandler } = await imp("pptx/index.mjs");
      open = (b) =>
        new PptxHandler().load(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
    }
  } else {
    const mod = await import(pathToFileURL(join(PKG_DIR, "nitro_xml_wasm.js")).href);
    mod.initSync({ module: readFileSync(join(PKG_DIR, "nitro_xml_wasm_bg.wasm")) });
    open = (b) => {
      const h = new mod.Handle(b);
      h.openDocument(name);
    };
  }
  const bytes = new Uint8Array(file);
  const base = vmHwmMiB();
  const t0 = performance.now();
  await open(bytes);
  const ms = performance.now() - t0;
  console.log(JSON.stringify({ ms, peakMiB: vmHwmMiB() - base }));
}

if (args.child) {
  await child(args.child);
} else {
  const runs = Number(args.runs ?? 5);
  const tiers = args.tier ? args.tier.split(",") : null;
  const formats = args.format ? args.format.split(",") : null;
  const rows = [];
  let rowIdx = 0;
  for (const [format, tier, rel] of FIXTURES) {
    if ((tiers && !tiers.includes(tier)) || (formats && !formats.includes(format))) continue;
    const row = { format, tier, fixture: rel, metric: "peak_rss_mib", runs };
    for (const engine of ["ts", "wasm"]) {
      const ms = [];
      const mem = [];
      for (let i = 0; i < runs; i++) {
        const out = execFileSync(
          process.execPath,
          [fileURLToPath(import.meta.url), `--child=${engine}:${rel}`, `--pkg=${PKG_DIR}`],
          { encoding: "utf8" },
        );
        const j = JSON.parse(out.trim().split("\n").pop());
        ms.push(j.ms);
        mem.push(j.peakMiB);
      }
      row[engine] = {
        ms: median(ms),
        mem: median(mem),
        ms_samples: ms.map((x) => +x.toFixed(3)),
        mem_samples: mem.map((x) => +x.toFixed(3)),
      };
    }
    const memRatio = row.wasm.mem / row.ts.mem;
    const memCi = bootstrapRatioCi(
      row.wasm.mem_samples,
      row.ts.mem_samples,
      BOOTSTRAP,
      SEED + rowIdx * 97,
    );
    row.wasm_over_ts_ms = row.wasm.ms / row.ts.ms;
    row.wasm_over_ts_mem = memRatio;
    row.wasm_over_ts_mem_ci95 = { lo: memCi.lo, hi: memCi.hi, bootstrap: memCi.B };
    row.verdict = verdict(memRatio, memCi);
    rows.push(row);
    rowIdx++;
  }
  if (args.json) {
    console.log(
      JSON.stringify(
        {
          schema: 1,
          race: "m1-peak-mem-vmhwm",
          method: {
            timedRuns: runs,
            bootstrap: BOOTSTRAP,
            seed: SEED,
            metric: "VmHWM growth over post-init baseline (MiB), fresh process per (engine, fixture, run)",
            ratio: "median(wasm)/median(ts); 95% CI via unpaired bootstrap of that median ratio",
            beats_rule:
              "WIN = median ratio < 1 AND CI upper < 1 (PRODUCT_SPEC §5); otherwise LOSS or TIE/LOSS",
          },
          rows,
        },
        null,
        2,
      ),
    );
  } else {
    console.log(
      "| Format | Tier | TS ms | WASM ms | WASM/TS ms | TS peak MiB | WASM peak MiB | WASM/TS mem | 95% CI mem | Verdict |",
    );
    console.log("|---|---|--:|--:|--:|--:|--:|--:|--:|---------|");
    for (const r of rows) {
      const ci = r.wasm_over_ts_mem_ci95;
      console.log(
        `| ${r.format} | ${r.tier} | ${r.ts.ms.toFixed(2)} | ${r.wasm.ms.toFixed(2)} | ${r.wasm_over_ts_ms.toFixed(3)} | ${r.ts.mem.toFixed(1)} | ${r.wasm.mem.toFixed(1)} | ${r.wasm_over_ts_mem.toFixed(3)} | [${ci.lo.toFixed(3)}, ${ci.hi.toFixed(3)}] | **${r.verdict}** |`,
      );
    }
  }
}
