// SPDX-License-Identifier: MIT
/**
 * Cold open-to-first-paint + JS heap peak for React embed (S/M/L × formats).
 * Usage: node measure-open-mem.mjs
 */
import { spawn } from "node:child_process";
import { writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 5188;
const BASE = `http://127.0.0.1:${PORT}`;

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

async function runOne(browser, format, tier) {
  const context = await browser.newContext();
  const page = await context.newPage();
  page.setDefaultTimeout(180000);
  const heaps = [];
  const timer = setInterval(async () => {
    try {
      const h = await heapUsed(page);
      if (typeof h === "number") heaps.push(h);
    } catch {
      /* page closed */
    }
  }, 50);

  const q = new URLSearchParams({ scenario: "first-paint", format, tier });
  await page.goto(`${BASE}/?${q}`, { waitUntil: "domcontentloaded", timeout: 60000 });
  await page.waitForFunction(() => window.__nitroBench && window.__nitroBench.state.done, null, {
    timeout: 180000,
  });
  const result = await page.evaluate(() => window.__nitroBench.state.result);
  const heap_after = await heapUsed(page);
  clearInterval(timer);
  if (typeof heap_after === "number") heaps.push(heap_after);
  await context.close().catch(() => {});

  const peak = heaps.length ? Math.max(...heaps) : null;
  return {
    format,
    tier,
    ok: !!result?.ok,
    open_ms: result?.open_ms ?? null,
    paint_ms: result?.paint_ms ?? null,
    mount_to_paint_ms: result?.mount_to_paint_ms ?? null,
    bytes: result?.bytes ?? null,
    heap_after_bytes: heap_after,
    heap_peak_bytes: peak,
    streaming_instantiate: result?.wasm?.streaming_instantiate ?? null,
    init_core_ms: result?.init_core_ms ?? null,
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
    ["xlsx", "S"],
    ["xlsx", "M"],
    ["xlsx", "L"],
    ["docx", "S"],
    ["docx", "M"],
    ["docx", "L"],
    ["pptx", "S"],
    ["pptx", "M"],
    ["pptx", "L"],
  ];

  const rows = [];
  try {
    for (const [format, tier] of matrix) {
      process.stderr.write(`… ${format} ${tier}\n`);
      const row = await runOne(browser, format, tier);
      rows.push(row);
      const mb = (n) => (typeof n === "number" ? (n / (1024 * 1024)).toFixed(1) : "—");
      console.log(
        `${format}/${tier}: mount→paint=${row.mount_to_paint_ms?.toFixed?.(1) ?? "—"}ms ` +
          `open=${row.open_ms?.toFixed?.(1) ?? "—"} paint=${row.paint_ms?.toFixed?.(1) ?? "—"} ` +
          `heap_peak=${mb(row.heap_peak_bytes)}MiB heap_after=${mb(row.heap_after_bytes)}MiB ` +
          `bytes=${row.bytes}`,
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
    rows,
  };
  const outPath = join(
    HERE,
    "results",
    `open-mem-${process.env.MEASURE_LABEL || "run"}.json`,
  );
  writeFileSync(outPath, JSON.stringify(out, null, 2));
  console.log(`Wrote ${outPath}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
