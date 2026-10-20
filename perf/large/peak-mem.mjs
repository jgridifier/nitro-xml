#!/usr/bin/env node
// SPDX-License-Identifier: MIT
/**
 * Peak-memory + cold-open table for TS (upstream ooxml-core) vs Nitro WASM, one fresh process per
 * (engine, fixture, run) so VmHWM (process-lifetime peak RSS) is attributable to a single open.
 *
 *   node perf/large/peak-mem.mjs [--runs=5] [--pkg=<dir with nitro_xml_wasm.js + _bg.wasm>]
 *        [--tier=S,M,L] [--format=xlsx,docx,pptx] [--json]
 *
 * Reports, per fixture: median open ms and median peak-RSS growth (MiB) over the post-init
 * baseline for each engine, plus the WASM/TS ratios. Child mode: `--child=<engine>:<fixture>`.
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

function median(xs) {
  const a = [...xs].sort((x, y) => x - y);
  return a[Math.floor(a.length / 2)];
}

if (args.child) {
  await child(args.child);
} else {
  const runs = Number(args.runs ?? 5);
  const tiers = args.tier ? args.tier.split(",") : null;
  const formats = args.format ? args.format.split(",") : null;
  const rows = [];
  for (const [format, tier, rel] of FIXTURES) {
    if ((tiers && !tiers.includes(tier)) || (formats && !formats.includes(format))) continue;
    const row = { format, tier };
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
      row[engine] = { ms: median(ms), mem: median(mem) };
    }
    rows.push(row);
  }
  if (args.json) {
    console.log(JSON.stringify(rows, null, 2));
  } else {
    console.log("| Format | Tier | TS ms | WASM ms | WASM/TS ms | TS peak MiB | WASM peak MiB | WASM/TS mem |");
    console.log("|---|---|--:|--:|--:|--:|--:|--:|");
    for (const r of rows) {
      console.log(
        `| ${r.format} | ${r.tier} | ${r.ts.ms.toFixed(2)} | ${r.wasm.ms.toFixed(2)} | ${(r.wasm.ms / r.ts.ms).toFixed(3)} | ${r.ts.mem.toFixed(1)} | ${r.wasm.mem.toFixed(1)} | ${(r.wasm.mem / r.ts.mem).toFixed(3)} |`,
      );
    }
  }
}
