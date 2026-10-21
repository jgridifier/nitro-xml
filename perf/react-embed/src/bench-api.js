// SPDX-License-Identifier: MIT
import fixtures from "../fixtures.json";

export { fixtures };

export function tagFor(format) {
  if (format === "xlsx") return "nitro-xlsx";
  if (format === "docx") return "nitro-docx";
  if (format === "pptx") return "nitro-pptx";
  throw new Error(`unknown format ${format}`);
}

export function percentile(samples, p) {
  if (!samples.length) return null;
  const a = [...samples].sort((x, y) => x - y);
  const idx = Math.min(a.length - 1, Math.max(0, Math.ceil((p / 100) * a.length) - 1));
  return a[idx];
}

export async function fetchFixture(format, tier) {
  const entry = fixtures[format]?.[tier];
  if (!entry) throw new Error(`no fixture for ${format}/${tier}`);
  const res = await fetch(entry.url);
  if (!res.ok) throw new Error(`fixture fetch ${entry.url}: ${res.status}`);
  const buf = await res.arrayBuffer();
  return { buf, name: entry.name, url: entry.url, bytes: buf.byteLength };
}

function probeStyleRecalc(el) {
  const t0 = performance.now();
  try {
    const host = el.shadowRoot || el;
    void host.querySelector?.("canvas")?.getBoundingClientRect?.();
    void el.offsetHeight;
  } catch {
    /* ignore */
  }
  return performance.now() - t0;
}

export async function waitFirstPaint(el, timeoutMs = 30000, trace = null) {
  const t0 = performance.now();
  const doc = el.document;
  if (!doc) throw new Error("no document after open");
  const tick0 = performance.now();
  await doc.tick();
  if (trace) {
    trace.set("first_tick_ms", performance.now() - tick0);
    window.__nitroBench?.mark("first_tick", { ms: trace.phases.first_tick_ms });
  }
  const deadline = t0 + timeoutMs;
  while (performance.now() < deadline) {
    const canvas = el.shadowRoot?.querySelector("canvas");
    const dom = el.shadowRoot?.querySelector(".dom");
    const hasDom = dom && dom.childElementCount > 0;
    let hasPixels = false;
    if (canvas && canvas.width > 0 && canvas.height > 0) {
      try {
        const ctx = canvas.getContext("2d");
        if (ctx) {
          const sample = ctx.getImageData(
            0,
            0,
            Math.min(8, canvas.width),
            Math.min(8, canvas.height),
          );
          hasPixels = sample.data.some((v, i) => i % 4 !== 3 && v !== 0);
        }
      } catch {
        // tainted canvas — treat presence of non-zero size as painted
        hasPixels = true;
      }
    }
    if (hasDom || hasPixels) {
      const paint_ms = performance.now() - t0;
      if (trace) {
        trace.set("paint_wait_ms", paint_ms);
        trace.set("style_recalc_probe_ms", probeStyleRecalc(el));
        window.__nitroBench?.mark("first_paint", { ms: paint_ms });
      }
      return paint_ms;
    }
    await doc.tick();
    await new Promise((r) => requestAnimationFrame(r));
  }
  throw new Error("first paint timeout");
}

export async function openInElement(el, format, tier, opts = {}) {
  const { beginTraceSession } = await import("./trace.js");
  const trace = opts.trace === false ? null : beginTraceSession();

  const tf0 = performance.now();
  const { buf, name, bytes } = await fetchFixture(format, tier);
  if (trace) {
    trace.set("fixture_fetch_ms", performance.now() - tf0);
    window.__nitroBench?.mark("fixture_fetch", { ms: trace.phases.fixture_fetch_ms, bytes });
  }

  const t0 = performance.now();
  await el.loadBuffer(buf, name);
  const open_ms = performance.now() - t0;
  if (trace) {
    trace.set("host_load_buffer_ms", open_ms);
    // Worker open_trace lands via mark from worker-host when trace:true.
    const marks = window.__nitroBench?.state?.marks || [];
    const wot = [...marks].reverse().find((m) => m.name === "worker_open_trace");
    if (wot) {
      // mark() stores extra fields on the entry itself
      const { name: _n, t: _t, ...rest } = wot;
      trace.phases.worker_open = rest;
    }
  }
  const paint_ms = await waitFirstPaint(el, 30000, trace);
  const mount_to_paint_ms = open_ms + paint_ms;
  if (trace) trace.set("mount_to_paint_ms", mount_to_paint_ms);
  return {
    open_ms,
    paint_ms,
    mount_to_paint_ms,
    bytes,
    name,
    ...(trace ? { breakdown: trace.snapshot() } : {}),
  };
}

export async function editAndRepaint(el, specOrFn) {
  const doc = el.document;
  if (!doc) throw new Error("no document");
  const t0 = performance.now();
  if (typeof specOrFn === "function") {
    await specOrFn(el);
  } else {
    await doc.applyEdit(specOrFn);
  }
  // tick applies the command buffer to canvas (= painted). Skip trailing rAF
  // so the metric is not floor-clamped to one display frame (~16ms).
  await doc.tick();
  return performance.now() - t0;
}

export function cellTypeSpec(i) {
  return JSON.stringify({
    command: "setCellInput",
    sheet: 0,
    row: 0,
    col: 0,
    text: `t${i}`,
  });
}

/** One printable keystroke via the editor-key path (docx/pptx caret typing). */
export async function typeKey(el, key = "a") {
  const doc = el.document;
  if (!doc || !doc.editorKey) throw new Error("no editorKey");
  const edit = await doc.editorKey(
    { key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false },
    false,
  );
  if (edit && el.shadowRoot) {
    const ch = el.shadowRoot.querySelector(".chrome");
    if (ch && (edit.h != null || edit.hf != null)) {
      if (edit.hf != null) {
        const x = ch.querySelector(".xfb");
        if (x) x.outerHTML = edit.hf;
        else ch.insertAdjacentHTML("afterbegin", edit.hf);
      } else if (edit.h != null) ch.innerHTML = edit.h;
    }
  }
}

export async function boldEdit(el) {
  await el.shellAction("cmd", "home.bold");
}
