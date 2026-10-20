// SPDX-License-Identifier: MIT
/**
 * Edit→paint latency + JS heap peak for React embed.
 * Matrix: xlsx cell, pptx type, bold (xlsx+docx+pptx) × S/L, n>=50.
 *
 *   MEASURE_LABEL=before node measure-edit-latency.mjs
 */
import { spawn } from "node:child_process";
import { writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 5199;
const BASE = `http://127.0.0.1:${PORT}`;
const N = Number(process.env.EDIT_SAMPLES || 50);

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function waitUrl(url, timeoutMs = 60000) {
  const t0 = Date.now();
  while (Date.now() - t0 < timeoutMs) {
    try {
      const res = await fetch(url);
      if (res.ok || res.status === 404) return;
    } catch {
      /* retry */
    }
    await sleep(200);
  }
  throw new Error(`server not up: ${url}`);
}

async function startVite() {
  const child = spawn(
    "npx",
    ["vite", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"],
    {
      cwd: HERE,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, BROWSER: "none" },
    },
  );
  let log = "";
  child.stdout.on("data", (d) => {
    log += d.toString();
  });
  child.stderr.on("data", (d) => {
    log += d.toString();
  });
  try {
    await waitUrl(BASE);
  } catch (e) {
    child.kill("SIGTERM");
    throw new Error(`${e.message}\nvite log:\n${log}`);
  }
  return child;
}

async function heapUsed(page) {
  return page.evaluate(() => {
    const m = performance.memory;
    return m ? m.usedJSHeapSize : null;
  });
}

async function runOne(browser, scenario, format, tier) {
  const context = await browser.newContext();
  const page = await context.newPage();
  page.setDefaultTimeout(300000);
  const heaps = [];
  const timer = setInterval(async () => {
    try {
      const h = await heapUsed(page);
      if (typeof h === "number") heaps.push(h);
    } catch {
      /* closed */
    }
  }, 50);

  const q = new URLSearchParams({
    scenario,
    format,
    tier,
    samples: String(N),
  });
  await page.goto(`${BASE}/?${q}`, { waitUntil: "domcontentloaded", timeout: 60000 });
  await page.waitForFunction(() => window.__nitroBench && window.__nitroBench.state.done, null, {
    timeout: 300000,
  });
  const result = await page.evaluate(() => window.__nitroBench.state.result);
  const heap_after = await heapUsed(page);
  clearInterval(timer);
  if (typeof heap_after === "number") heaps.push(heap_after);
  await context.close().catch(() => {});

  const edit = result?.edit || {};
  return {
    scenario,
    format,
    tier,
    ok: !!result?.ok,
    n: edit.n ?? null,
    p50_ms: edit.p50_ms ?? null,
    p95_ms: edit.p95_ms ?? null,
    heap_peak_bytes: heaps.length ? Math.max(...heaps) : null,
    heap_after_bytes: heap_after,
    error: result?.error || null,
  };
}

async function main() {
  if (!existsSync(join(HERE, "node_modules/vite"))) {
    console.error("Run npm install in perf/react-embed first");
    process.exit(1);
  }
  const child = await startVite();
  const browser = await chromium.launch({
    headless: true,
    args: ["--disable-lcd-text", "--no-sandbox", "--enable-precise-memory-info"],
  });

  const matrix = [
    ["edit-cell", "xlsx", "S"],
    ["edit-cell", "xlsx", "L"],
    ["edit-pptx-text", "pptx", "S"],
    ["edit-pptx-text", "pptx", "L"],
    ["edit-bold", "xlsx", "S"],
    ["edit-bold", "xlsx", "L"],
    ["edit-bold", "docx", "S"],
    ["edit-bold", "docx", "L"],
    ["edit-bold", "pptx", "S"],
    ["edit-bold", "pptx", "L"],
  ];

  const rows = [];
  try {
    for (const [scenario, format, tier] of matrix) {
      process.stderr.write(`… ${scenario} ${format}/${tier} n=${N}\n`);
      const row = await runOne(browser, scenario, format, tier);
      rows.push(row);
      const mb = (n) => (typeof n === "number" ? (n / (1024 * 1024)).toFixed(1) : "—");
      console.log(
        `${scenario} ${format}/${tier}: p50=${row.p50_ms?.toFixed?.(2) ?? "—"} ` +
          `p95=${row.p95_ms?.toFixed?.(2) ?? "—"} n=${row.n} ` +
          `heap_peak=${mb(row.heap_peak_bytes)}MiB ok=${row.ok}` +
          (row.error ? ` err=${row.error}` : ""),
      );
    }
  } finally {
    await browser.close().catch(() => {});
    child.kill("SIGTERM");
    await sleep(300);
  }

  mkdirSync(join(HERE, "results"), { recursive: true });
  const out = {
    when: new Date().toISOString(),
    label: process.env.MEASURE_LABEL || "run",
    n: N,
    rows,
  };
  const outPath = join(
    HERE,
    "results",
    `edit-latency-${process.env.MEASURE_LABEL || "run"}.json`,
  );
  writeFileSync(outPath, JSON.stringify(out, null, 2));
  console.log(`Wrote ${outPath}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
