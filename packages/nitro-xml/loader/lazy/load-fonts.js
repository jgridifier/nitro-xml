// SPDX-License-Identifier: MIT
/**
 * Font loading helpers (lazy — excluded from the eager brotli budget).
 * loadFontsOnMainThread: main thread → FontFace + document.fonts.
 * loadFontsInWorker:     worker context → FontFace + self.fonts.
 */
import { _fd } from "../font-source.js";

/**
 * Normalise a font entry to { family, src, weight, style } or null.
 * src is a CSS font source string accepted by new FontFace(family, src, {...}).
 */
function normEntry(entry) {
  if (!entry || typeof entry !== "object") return null;
  const { family, url, weight, style } = entry;
  if (!family || !url) return null;
  const s = String(url);
  // local() and url() are valid CSS font source strings as-is
  const src =
    s.startsWith("local(") || s.startsWith("url(") || s.startsWith("data:")
      ? s
      : `url("${s}")`;
  return { family, src, weight: weight || "normal", style: style || "normal" };
}

/**
 * Load a fonts array on the main thread using FontFace.
 * Returns serialized descriptors ({ family, src, weight, style }) for the worker.
 * stylesheet-URL strings (non-object entries) are injected as <link> elements.
 */
export async function lfm(fonts) {
  if (!fonts || !fonts.length) return [];
  const descriptors = [];
  for (const entry of fonts) {
    if (typeof entry === "string") {
      await _loadStylesheet(entry);
      continue;
    }
    const norm = normEntry(entry);
    if (!norm) continue;
    if (typeof FontFace === "undefined") {
      descriptors.push(norm);
      continue;
    }
    try {
      const face = new FontFace(norm.family, norm.src, {
        weight: norm.weight,
        style: norm.style,
      });
      await face.load();
      if (typeof document !== "undefined" && document.fonts) document.fonts.add(face);
      descriptors.push(norm);
    } catch (e) {
      // Load failure is non-fatal — skip this face.
      if (typeof console !== "undefined") console.warn("[nitro-xml] font load:", norm.family, e);
    }
  }
  return descriptors;
}

function _loadStylesheet(href) {
  if (typeof document === "undefined" || !document.head) return Promise.resolve();
  const q = `link[rel="stylesheet"][href="${href}"]`;
  if (document.head.querySelector(q)) return Promise.resolve();
  return new Promise((resolve) => {
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = href;
    link.onload = resolve;
    link.onerror = resolve;
    document.head.appendChild(link);
  });
}

/**
 * Load serialized font descriptors inside a worker via self.fonts.
 * Called from worker.js via dynamic import to keep worker.js small.
 */
export async function lfw(fonts) {
  if (!fonts || !fonts.length || typeof FontFace === "undefined" || !self.fonts) return;
  for (const entry of fonts) {
    const f = entry && typeof entry === "object" ? (entry.src ? entry : normEntry(entry)) : null;
    if (!f) continue;
    try {
      const face = new FontFace(f.family, f.src, { weight: f.weight || "normal", style: f.style || "normal" });
      await face.load();
      self.fonts.add(face);
    } catch {}
  }
}

/** Prepare fonts + serializable fontSource for WorkerDocument._openBytes (main thread). */
export async function prep(fonts, fontSource) {
  const f = fonts ?? _fd.f;
  const s = typeof fontSource == "function" || fontSource == "page" ? "off" : fontSource;
  if (f?.length) await lfm(f);
  return [f, s];
}

/** Load fonts from an open-message in the worker context. */
export async function wof(msg) {
  if (msg.fonts?.length) await lfw(msg.fonts);
}
