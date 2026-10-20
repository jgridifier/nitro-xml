// SPDX-License-Identifier: MIT
/**
 * Playwright driver for the React embed latency bench.
 *
 *   node run.mjs            # full suite
 *   node run.mjs --smoke    # startup + xlsx S first-paint
 */
import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const HERE = dirname(fileURLToPath(import.meta.url));
const PORT = 5179;
const BASE = `http://127.0.0.1:${PORT}`;
const smoke = process.argv.includes("--smoke");

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
  const child = spawn("npx", ["vite", "--port", String(PORT), "--strictPort", "--host", "127.0.0.1"], {
    cwd: HERE,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, BROWSER: "none" },
  });
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
  return { child, log: () => log };
}

async function runScenario(page, scenario, extra = {}) {
  const q = new URLSearchParams({ scenario, ...extra });
  if (smoke) q.set("smoke", "1");
  await page.goto(`${BASE}/?${q}`, { waitUntil: "domcontentloaded", timeout: 60000 });
  await page.waitForFunction(() => window.__nitroBench && window.__nitroBench.state.done, null, {
    timeout: smoke ? 90000 : 180000,
  });
  return page.evaluate(() => window.__nitroBench.state.result);
}

function mdTable(rows, headers) {
  const lines = [
    `| ${headers.join(" | ")} |`,
    `| ${headers.map(() => "---").join(" | ")} |`,
  ];
  for (const r of rows) lines.push(`| ${headers.map((h) => r[h] ?? "").join(" | ")} |`);
  return lines.join("\n");
}

async function main() {
  if (!existsSync(join(HERE, "node_modules/vite"))) {
    console.error("Run npm install in perf/react-embed first");
    process.exit(1);
  }
  const { child } = await startVite();
  const browser = await chromium.launch({
    headless: true,
    args: ["--disable-lcd-text", "--no-sandbox"],
  });
  const results = {
    when: new Date().toISOString(),
    smoke,
    scenarios: {},
  };

  try {
    async function withPage(fn) {
      const page = await browser.newPage();
      page.setDefaultTimeout(smoke ? 90000 : 180000);
      page.on("pageerror", (e) => console.error("[pageerror]", e.message));
      try {
        return await fn(page);
      } finally {
        await page.close().catch(() => {});
      }
    }

    // Cold startup (fresh context — no prior wasm/worker)
    results.scenarios.startup = await withPage((page) => runScenario(page, "startup"));

    const formats = smoke
      ? [["xlsx", "S"]]
      : [
          ["xlsx", "S"],
          ["xlsx", "M"],
          ["docx", "S"],
          ["docx", "M"],
          ["pptx", "S"],
          ["pptx", "M"],
        ];

    for (const [format, tier] of formats) {
      const key = `first-paint:${format}:${tier}`;
      results.scenarios[key] = await withPage((page) =>
        runScenario(page, "first-paint", { format, tier }),
      );
    }

    if (!smoke) {
      results.scenarios["edit-cell"] = await withPage((page) =>
        runScenario(page, "edit-cell", { format: "xlsx", samples: "20" }),
      );
      results.scenarios["edit-docx-text"] = await withPage((page) =>
        runScenario(page, "edit-docx-text", { format: "docx", samples: "20" }),
      );
      results.scenarios["edit-pptx-text"] = await withPage((page) =>
        runScenario(page, "edit-pptx-text", { format: "pptx", samples: "20" }),
      );
      results.scenarios["edit-bold"] = await withPage((page) =>
        runScenario(page, "edit-bold", { format: "xlsx", samples: "10" }),
      );
      results.scenarios.remount = await withPage((page) =>
        runScenario(page, "remount", { format: "xlsx", tier: "S" }),
      );
      results.scenarios["strict-double-mount"] = await withPage((page) =>
        runScenario(page, "strict-double-mount", {
          format: "xlsx",
          tier: "S",
          strict: "1",
        }),
      );
    }

    mkdirSync(join(HERE, "results"), { recursive: true });
    const outPath = join(HERE, "results", smoke ? "smoke.json" : "latest.json");
    writeFileSync(outPath, JSON.stringify(results, null, 2));

    // Markdown summary
    const paintRows = Object.entries(results.scenarios)
      .filter(([k]) => k.startsWith("first-paint:"))
      .map(([k, v]) => {
        const [, format, tier] = k.split(":");
        return {
          Format: format,
          Tier: tier,
          "open ms": v.open_ms?.toFixed?.(1) ?? v.open_ms,
          "paint ms": v.paint_ms?.toFixed?.(1) ?? v.paint_ms,
          "mount→paint ms": v.mount_to_paint_ms?.toFixed?.(1) ?? v.mount_to_paint_ms,
          bytes: v.bytes,
        };
      });

    const s = results.scenarios.startup || {};
    console.log("\n## React embed bench\n");
    console.log(
      `WASM fetch ${s.wasm?.fetch_ms?.toFixed?.(1) ?? "—"} · compile ${s.wasm?.compile_ms?.toFixed?.(1) ?? "—"} · instantiate ${s.wasm?.instantiate_ms?.toFixed?.(1) ?? "—"} · worker ${s.worker?.construct_ms?.toFixed?.(1) ?? "—"} · initCore ${s.init_core_ms?.toFixed?.(1) ?? "—"}`,
    );
    console.log(
      `streaming compile=${s.wasm?.streaming_compile} instantiate=${s.wasm?.streaming_instantiate}`,
    );
    if (paintRows.length) {
      console.log("\n### Mount → first paint\n");
      console.log(mdTable(paintRows, ["Format", "Tier", "open ms", "paint ms", "mount→paint ms", "bytes"]));
    }
    for (const key of ["edit-cell", "edit-docx-text", "edit-pptx-text", "edit-bold"]) {
      const e = results.scenarios[key]?.edit;
      if (!e) continue;
      console.log(`\n${key}: p50=${e.p50_ms?.toFixed?.(2)} p95=${e.p95_ms?.toFixed?.(2)} (n=${e.n})`);
    }
    if (results.scenarios.remount) {
      const r = results.scenarios.remount;
      console.log(
        `\nremount: first=${r.first?.mount_to_paint_ms?.toFixed?.(1)} second=${r.second?.mount_to_paint_ms?.toFixed?.(1)} delta=${r.remount_delta_ms?.toFixed?.(1)}`,
      );
    }
    if (results.scenarios["strict-double-mount"]) {
      const r = results.scenarios["strict-double-mount"];
      console.log(
        `strict-double-mount: host_ready_count=${r.host_ready_count} gaps=${JSON.stringify(r.host_ready_gaps_ms)} mount→paint=${r.mount_to_paint_ms?.toFixed?.(1)}`,
      );
    }
    console.log(`\nWrote ${outPath}`);
  } finally {
    await browser.close().catch(() => {});
    child.kill("SIGTERM");
    await sleep(300);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
