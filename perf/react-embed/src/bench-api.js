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

export async function waitFirstPaint(el, timeoutMs = 30000) {
  const t0 = performance.now();
  const doc = el.document;
  if (!doc) throw new Error("no document after open");
  // Drive at least one frame; then wait until canvas/DOM shows content.
  await doc.tick();
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
      return performance.now() - t0;
    }
    await doc.tick();
    await new Promise((r) => requestAnimationFrame(r));
  }
  throw new Error("first paint timeout");
}

export async function openInElement(el, format, tier) {
  const { buf, name, bytes } = await fetchFixture(format, tier);
  const t0 = performance.now();
  await el.loadBuffer(buf, name);
  const open_ms = performance.now() - t0;
  const paint_ms = await waitFirstPaint(el);
  return { open_ms, paint_ms, mount_to_paint_ms: open_ms + paint_ms, bytes, name };
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
  await doc.tick();
  await new Promise((r) => requestAnimationFrame(r));
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
  if (edit && edit.h != null && el.shadowRoot) {
    const ch = el.shadowRoot.querySelector(".chrome");
    if (ch) ch.innerHTML = edit.h;
  }
}

export async function boldEdit(el) {
  await el.shellAction("cmd", "home.bold");
}
