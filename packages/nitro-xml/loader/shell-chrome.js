// SPDX-License-Identifier: MIT
/** Thin Stage-6 shell chrome binder (#29). Logic lives in Rust `shellAction` / `shellUi`. */

const CSS =
  `:host{display:flex;flex-direction:column;min-height:12rem;background:#fff;color:#111}` +
  `.bar,.tabs,.find,.diag{display:flex;flex-wrap:wrap;gap:.35rem;align-items:center;` +
  `padding:.3rem .5rem;border-bottom:1px solid #ddd;font:12px/1.3 ui-sans-serif,system-ui,sans-serif}` +
  `.bar button,.tabs button,.find button{font:inherit;padding:.2rem .45rem;cursor:pointer}` +
  `.tabs button[aria-current="true"]{font-weight:700}` +
  `.find[hidden],.diag[hidden],.msg[hidden]{display:none!important}` +
  `.find input{flex:1;min-width:8rem;font:inherit}` +
  `.viewport{position:relative;flex:1;min-height:0}` +
  `canvas,.dom{position:absolute;inset:0;width:100%;height:100%}` +
  `.diag{align-items:flex-start;white-space:pre-wrap;max-height:9rem;overflow:auto;background:#f7f7f7}` +
  `.msg{padding:1rem;font:14px/1.4 ui-sans-serif,system-ui,sans-serif}`;

/** @param {ShadowRoot} shadow */
export function mountShellChrome(shadow) {
  shadow.innerHTML =
    `<style>${CSS}</style>` +
    `<div class="bar" part="toolbar">` +
    `<button type="button" data-act="zoom-out" title="Zoom out">−</button>` +
    `<span class="zoom" part="zoom">100%</span>` +
    `<button type="button" data-act="zoom-in" title="Zoom in">+</button>` +
    `<button type="button" data-act="find-open" title="Find">Find</button>` +
    `<button type="button" data-act="copy" title="Copy">Copy</button>` +
    `<button type="button" data-act="diagnostics-toggle" title="Diagnostics">Diag</button>` +
    `</div>` +
    `<div class="find" part="find" hidden>` +
    `<input type="search" class="q" part="find-query" placeholder="Find in workbook" aria-label="Find" />` +
    `<button type="button" data-act="find-prev" title="Previous">↑</button>` +
    `<button type="button" data-act="find-next" title="Next">↓</button>` +
    `<span class="find-status" part="find-status"></span>` +
    `<button type="button" data-act="find-close" title="Close find">✕</button>` +
    `</div>` +
    `<div class="tabs" part="tabs" role="tablist"></div>` +
    `<div class="viewport" part="viewport">` +
    `<canvas part="canvas"></canvas><div class="dom" part="dom"></div>` +
    `</div>` +
    `<div class="diag" part="diagnostics" hidden></div>` +
    `<div class="msg" hidden part="message"></div>`;
  return {
    bar: shadow.querySelector(".bar"),
    find: shadow.querySelector(".find"),
    findInput: shadow.querySelector(".q"),
    findStatus: shadow.querySelector(".find-status"),
    zoom: shadow.querySelector(".zoom"),
    tabs: shadow.querySelector(".tabs"),
    viewport: shadow.querySelector(".viewport"),
    canvas: shadow.querySelector("canvas"),
    root: shadow.querySelector(".dom"),
    diag: shadow.querySelector(".diag"),
    msg: shadow.querySelector(".msg"),
  };
}

/** Sync chrome from a Rust `shellUi` / action result UI object. */
export function bindShellUi(chrome, ui) {
  if (!chrome || !ui) return;
  if (chrome.zoom) chrome.zoom.textContent = `${Math.round(ui.zoom || 100)}%`;
  if (chrome.find) chrome.find.hidden = !ui.find?.open;
  if (chrome.findInput && document.activeElement !== chrome.findInput) {
    chrome.findInput.value = ui.find?.query || "";
  }
  if (chrome.findStatus) chrome.findStatus.textContent = ui.find?.status || "";
  if (chrome.tabs && Array.isArray(ui.tabs)) {
    const html = ui.tabs
      .map(
        (t) =>
          `<button type="button" role="tab" data-act="sheet" data-arg="${t.index}"` +
          `${t.active ? ' aria-current="true"' : ""}>${escapeText(t.name)}</button>`,
      )
      .join("");
    if (chrome.tabs.innerHTML !== html) chrome.tabs.innerHTML = html;
  }
  if (chrome.diag) {
    chrome.diag.hidden = !ui.diagnostics?.open;
    if (ui.diagnostics?.open) {
      const d = ui.diagnostics;
      const lines = [
        `format: ${ui.format || ""}`,
        `parse: ${fmtMs(d.parseMs)} · paint: ${fmtMs(d.paintMs)}`,
        `warnings: ${(d.warnings || []).length}`,
        `font substitutions (#121): ${(d.fontSubstitutions || []).length}`,
        `unsupported: ${(d.unsupported || []).length}`,
      ];
      for (const item of d.fontSubstitutions || []) {
        lines.push(`· font: ${item.message || item.code}`);
      }
      for (const item of d.warnings || []) {
        lines.push(`· ${item.code}: ${item.message}`);
      }
      for (const item of d.unsupported || []) {
        lines.push(`· unsupported: ${item.message || item.code}`);
      }
      chrome.diag.textContent = lines.join("\n");
    }
  }
}

function fmtMs(v) {
  const n = Number(v);
  return Number.isFinite(n) ? `${n.toFixed(1)} ms` : "—";
}

function escapeText(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Open a shell hyperlink respecting safe-href `open` and internal `location`. */
export function followHyperlink(link) {
  if (!link) return;
  if (link.location) {
    // Internal workbook jump: host may listen for `nitro-hyperlink`.
    return { kind: "internal", location: link.location };
  }
  if (link.open && link.target) {
    try {
      window.open(link.target, "_blank", "noopener,noreferrer");
    } catch {
      /* ignore */
    }
    return { kind: "external", target: link.target };
  }
  return { kind: "blocked", target: link.target || null };
}
