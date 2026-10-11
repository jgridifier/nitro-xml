// SPDX-License-Identifier: MIT
/**
 * Boundary transport microbench for #14.
 * Test/bench only — never shipped. Numbers land in docs/boundary.md and
 * perf/micro/results/ (history ingestion is TODO(#8)).
 *
 * Usage:
 *   bash scripts/build-wasm.sh
 *   cd perf/micro && npm ci && npm run bench
 */
import { chromium } from "playwright-core";
import { createServer } from "node:http";
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "../..");
const PKG = join(ROOT, "packages/nitro-xml/pkg");
const SIZES = [1_000, 100_000, 1_000_000];
const WARMUP = 2;
const RUNS = 7;

function median(xs) {
  const a = [...xs].sort((x, y) => x - y);
  const m = Math.floor(a.length / 2);
  return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2;
}

function contentType(p) {
  switch (extname(p)) {
    case ".js":
      return "text/javascript";
    case ".wasm":
      return "application/wasm";
    case ".html":
      return "text/html";
    default:
      return "application/octet-stream";
  }
}

const harnessHtml = `<!doctype html>
<meta charset="utf-8" />
<title>boundary bench</title>
<script type="module">
try {
  const mod = await import("/pkg/nitro_xml_wasm.js");
  const exports = await mod.default();
  window.__nitro = { Handle: mod.Handle, memory: exports.memory };
  window.__err = null;
} catch (e) {
  window.__err = String(e && (e.stack || e));
}
window.__ready = true;
</script>
`;

const coop = {
  "cross-origin-opener-policy": "same-origin",
  "cross-origin-embedder-policy": "require-corp",
  "cache-control": "no-store",
};

async function withServer(fn) {
  const server = createServer((req, res) => {
    const url = req.url === "/" ? "/index.html" : req.url;
    if (url === "/index.html") {
      res.writeHead(200, { "content-type": "text/html", ...coop });
      res.end(harnessHtml);
      return;
    }
    if (url.startsWith("/pkg/")) {
      const file = join(PKG, url.slice("/pkg/".length));
      if (!existsSync(file)) {
        res.writeHead(404, coop);
        res.end("missing");
        return;
      }
      res.writeHead(200, {
        "content-type": contentType(file),
        "cross-origin-resource-policy": "same-origin",
        ...coop,
      });
      res.end(readFileSync(file));
      return;
    }
    res.writeHead(404, coop);
    res.end("no");
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const { port } = server.address();
  try {
    return await fn(`http://127.0.0.1:${port}/`);
  } finally {
    server.close();
  }
}

async function time(page, label, fnSource) {
  const samples = [];
  for (let i = 0; i < WARMUP + RUNS; i++) {
    const ms = await page.evaluate(fnSource);
    if (i >= WARMUP) samples.push(ms);
  }
  return { label, median_ms: median(samples), samples };
}

async function main() {
  if (!existsSync(join(PKG, "nitro_xml_wasm.js"))) {
    console.error("missing packages/nitro-xml/pkg; run: bash scripts/build-wasm.sh");
    process.exit(1);
  }

  let commit = "unknown";
  try {
    commit = execSync("git rev-parse --short HEAD", { cwd: ROOT }).toString().trim();
  } catch {
    /* ignore */
  }

  const result = await withServer(async (base) => {
    const browser = await chromium.launch({
      executablePath: process.env.CHROME_PATH || "/usr/local/bin/google-chrome",
      headless: true,
      args: ["--enable-features=WebAssemblySimd"],
    });
    const page = await browser.newPage();
    await page.goto(base, { waitUntil: "networkidle" });
    await page.waitForFunction(() => window.__ready === true);
    const bootErr = await page.evaluate(() => window.__err);
    if (bootErr) throw new Error(`wasm boot failed: ${bootErr}`);

    const chromeVersion = await page.evaluate(() => navigator.userAgent);
    const rows = [];

    for (const n of SIZES) {
      // Prepare grid once per size (fill not timed for the transfer arms).
      await page.evaluate((n) => {
        if (window.__h) window.__h.free();
        window.__h = new window.__nitro.Handle(new Uint8Array(16));
        window.__h.fillSyntheticGrid(n);
      }, n);

      const soa = await time(
        page,
        `soa_view_scan_${n}`,
        () => {
          const h = window.__h;
          const mem = window.__nitro.memory.buffer;
          const t0 = performance.now();
          const values = new Float64Array(mem, h.gridValuePtr(), h.gridLen);
          const rows = new Uint32Array(mem, h.gridRowPtr(), h.gridLen);
          let sum = 0;
          for (let i = 0; i < values.length; i++) sum += values[i] + rows[i];
          const t1 = performance.now();
          if (sum === Infinity) throw new Error("unreachable");
          return t1 - t0;
        },
      );

      const serde = await time(
        page,
        `serde_wasm_bindgen_${n}`,
        () => {
          const h = window.__h;
          const t0 = performance.now();
          const records = h.exportRecordsSerde();
          let sum = 0;
          for (let i = 0; i < records.length; i++) sum += records[i].value + records[i].row;
          const t1 = performance.now();
          if (sum === Infinity) throw new Error("unreachable");
          return t1 - t0;
        },
      );

      const json = await time(
        page,
        `json_string_${n}`,
        () => {
          const h = window.__h;
          const t0 = performance.now();
          const records = JSON.parse(h.exportRecordsJson());
          let sum = 0;
          for (let i = 0; i < records.length; i++) sum += records[i].value + records[i].row;
          const t1 = performance.now();
          if (sum === Infinity) throw new Error("unreachable");
          return t1 - t0;
        },
      );

      rows.push({ n, soa, serde, json });
      console.log(
        `n=${n}  SoA ${soa.median_ms.toFixed(3)} ms  serde ${serde.median_ms.toFixed(3)} ms  json ${json.median_ms.toFixed(3)} ms`,
      );
    }

    // Command buffer vs per-call on a 1M-op / 1M-touch workload.
    await page.evaluate(() => {
      if (window.__h) window.__h.free();
      window.__h = new window.__nitro.Handle(new Uint8Array(8));
      window.__h.fillSyntheticGrid(1_000_000);
    });

    const cmdbuf = await time(page, "command_buffer_1m_ops", () => {
      const h = window.__h;
      const t0 = performance.now();
      h.encodeDemoFrame(1_000_000);
      const bytes = h.takeFrameCommands();
      const t1 = performance.now();
      if (bytes.length < 1_000_000) throw new Error("short buffer");
      return t1 - t0;
    });

    // Per-call contrast: 50k touches only (1M would take minutes and drown the table).
    const perCallN = 50_000;
    const perCall = await time(page, `per_call_touch_${perCallN}`, () => {
      const h = window.__h;
      const t0 = performance.now();
      let sum = 0;
      for (let i = 0; i < 50_000; i++) sum += h.touchCell(i);
      const t1 = performance.now();
      if (sum === Infinity) throw new Error("unreachable");
      return t1 - t0;
    });

    // Extrapolate per-call to 1M for the decision table (linear; documented as estimate).
    const perCall1mEstimate = (perCall.median_ms / perCallN) * 1_000_000;

    // Event batch round-trip smoke (not a transport race).
    const eventsOk = await page.evaluate(() => {
      const h = window.__h;
      // version=1, count=1, kind=Resize
      const buf = new ArrayBuffer(4 + 12 + 12);
      const v = new DataView(buf);
      v.setUint16(0, 1, true);
      v.setUint16(2, 1, true);
      v.setUint8(4, 6); // resize
      v.setUint8(5, 0);
      v.setUint16(6, 0, true);
      v.setFloat64(8, 1.0, true);
      v.setFloat32(16, 800, true);
      v.setFloat32(20, 600, true);
      v.setFloat32(24, 2, true);
      const n = h.pushEventBatch(new Uint8Array(buf));
      return n === 1 && h.pendingEventCount === 1 && h.drainEvents() === 1;
    });

    await browser.close();
    return {
      chromeVersion,
      cmdbuf,
      perCall,
      perCall1mEstimate,
      eventsOk,
      rows,
    };
  });

  const out = {
    commit,
    date: new Date().toISOString().slice(0, 10),
    chrome: result.chromeVersion,
    warmup: WARMUP,
    runs: RUNS,
    unit: "median_ms",
    transfer: result.rows.map((r) => ({
      n: r.n,
      soa_view_scan: r.soa.median_ms,
      serde_wasm_bindgen: r.serde.median_ms,
      json_string: r.json.median_ms,
    })),
    command_buffer_1m_ops: result.cmdbuf.median_ms,
    per_call_touch_50k: result.perCall.median_ms,
    per_call_1m_estimate_ms: result.perCall1mEstimate,
    events_ok: result.eventsOk,
  };

  const resultsDir = join(__dirname, "results");
  mkdirSync(resultsDir, { recursive: true });
  const outPath = join(resultsDir, `boundary-${out.date}-${commit}.json`);
  writeFileSync(outPath, JSON.stringify(out, null, 2) + "\n");
  writeFileSync(join(resultsDir, "boundary-latest.json"), JSON.stringify(out, null, 2) + "\n");

  console.log("\n## Markdown table\n");
  console.log("| N | SoA view+scan | serde-wasm-bindgen | JSON string |");
  console.log("|---|---:|---:|---:|");
  for (const r of out.transfer) {
    console.log(
      `| ${r.n.toLocaleString("en-US")} | ${r.soa_view_scan.toFixed(3)} ms | ${r.serde_wasm_bindgen.toFixed(3)} ms | ${r.json_string.toFixed(3)} ms |`,
    );
  }
  console.log(
    `\nCommand buffer 1M ops: ${out.command_buffer_1m_ops.toFixed(3)} ms` +
      `\nPer-call touch 50k: ${out.per_call_touch_50k.toFixed(3)} ms` +
      ` (≈ ${out.per_call_1m_estimate_ms.toFixed(0)} ms at 1M, linear estimate)` +
      `\nEvents ok: ${out.events_ok}` +
      `\nWrote ${outPath}`,
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
