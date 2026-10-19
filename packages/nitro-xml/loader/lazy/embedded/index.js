// SPDX-License-Identifier: MIT
// Embedded-mode toolbar (PowerPoint-Home look). Loaded by dynamic import() only when
// mode="embedded"; excluded from the first-load budget with the rest of loader/lazy/.
// Calls only the existing host.shellAction(op, arg); enabled/pressed state is mirrored
// from the (hidden) Rust chrome, which stays the single source of truth.
import { ICONS } from "./icons.js";
import { MANIFEST } from "./manifest.js";
import { CSS } from "./style.js";

const svg = (i) => `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[i] || ""}</svg>`;
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => "&#" + c.charCodeAt(0) + ";");

/** Mount (on=true) or remove (on=false) the embedded toolbar on a shell host element. */
export function setMode(host, on) {
  const cur = host._emb;
  if (!on) return cur && cur.destroy();
  if (cur) return;
  const spec = MANIFEST[host.localName.replace("nitro-", "")];
  if (!spec || !host._ch) return;
  host._emb = build(host, spec);
}

function build(host, spec) {
  const root = host.shadowRoot;
  const chrome = host._ch;
  const style = document.createElement("style");
  style.textContent = CSS;
  const bar = document.createElement("div");
  bar.className = "nx";
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", spec.label);
  bar.setAttribute("part", "embedded-toolbar");
  bar.dataset.fmt = host.localName.slice(6);
  const btns = [];
  const reads = [];
  let html = "";
  spec.groups.forEach((g, gi) => {
    html += `<div class="nx-g" role="group" aria-label="${esc(g.id)}" part="embedded-group" data-g="${gi}"><div class="nx-row">`;
    for (const it of g.items) {
      if (it.r) html += `<span class="nx-r" data-r="${it.r}" role="status"></span>`;
      else {
        const tg = it.p ? ' aria-pressed="false"' : "";
        html += `<button type="button" class="nx-b" part="embedded-button" tabindex="-1" aria-label="${esc((it.t || it.l).replace(/ \(.*/, ""))}" title="${esc(it.t || it.l)}" data-i="${btns.length}"${tg}>${svg(it.i)}<span class="nx-l" aria-hidden="true">${esc(it.l)}</span></button>`;
        btns.push(it);
      }
    }
    html += `</div><span class="nx-cap" aria-hidden="true">${esc(g.id)}</span></div>`;
  });
  html += `<div class="nx-more" hidden><button type="button" class="nx-b nx-mb" part="embedded-button" tabindex="-1" aria-label="More" title="More" aria-haspopup="true" aria-expanded="false">${svg("more")}<span class="nx-l" aria-hidden="true">More</span></button><div class="nx-pop" role="group" aria-label="More tools" hidden></div></div>`;
  bar.innerHTML = html;
  root.insertBefore(style, chrome);
  root.insertBefore(bar, chrome);
  host.setAttribute("data-nx", bar.dataset.fmt);

  const els = [...bar.querySelectorAll(".nx-b:not(.nx-mb)")];
  const groups = [...bar.querySelectorAll(".nx-g")];
  const more = bar.querySelector(".nx-more");
  const mb = bar.querySelector(".nx-mb");
  const pop = bar.querySelector(".nx-pop");
  const q = (s) => chrome.querySelector(s);

  const tabs = () => [...chrome.querySelectorAll(".t button")];
  const mirror = (it) => (it.c ? q(`[data-a="cmd"][data-g="${it.c}"]`) : null);

  function sync() {
    const tb = tabs();
    const at = tb.findIndex((b) => b.getAttribute("aria-current") === "true");
    const findOpen = !!q(".f") && !q(".f").hidden;
    btns.forEach((it, k) => {
      const el = els[k];
      let on = true;
      let dis = false;
      let pressed = null;
      if (it.c) {
        const m = mirror(it);
        on = !!m;
        dis = !m || m.disabled;
        pressed = m && m.getAttribute("aria-pressed");
      } else if (it.n) {
        const to = at + it.n;
        dis = at < 0 || to < 0 || to >= tb.length;
      } else if (it.x) pressed = String(findOpen);
      el.hidden = !on;
      el.disabled = dis;
      if (it.p) el.setAttribute("aria-pressed", pressed === "true" ? "true" : "false");
    });
    for (const r of bar.querySelectorAll(".nx-r")) {
      r.textContent =
        r.dataset.r === "zoom"
          ? (q(".z") || {}).textContent || ""
          : tb.length ? `${at + 1} / ${tb.length}` : "";
      r.hidden = !r.textContent;
    }
    for (const g of groups) g.hidden = !g.querySelector(".nx-b:not([hidden]), .nx-r:not([hidden])");
    fit();
    rove();
  }

  // ---- responsive: labels, then groups into the overflow menu ----
  const over = () => bar.scrollWidth > bar.clientWidth + 1;
  let sig = "";
  function layout() {
    pop.hidden = true;
    mb.setAttribute("aria-expanded", "false");
    more.hidden = true;
    for (const g of groups) bar.insertBefore(g, more);
    if (!bar.clientWidth) return;
    for (const s of ["wide", "compact"]) {
      bar.dataset.size = s;
      if (!over()) return;
    }
    more.hidden = false;
    const live = groups.filter((g) => !g.hidden);
    for (let k = live.length - 2; k > 0 && over(); k--) pop.prepend(live[k]);
  }
  function fit() {
    const s = [bar.clientWidth, ...groups.map((g) => +g.hidden), ...[...bar.querySelectorAll(".nx-r")].map((r) => r.textContent)].join();
    if (s === sig) return;
    sig = s;
    const a = root.activeElement;
    layout();
    a && a.isConnected && a.offsetParent && a.focus();
  }

  // ---- roving tabindex ----
  const items = () => [...bar.querySelectorAll(".nx-b")].filter((b) => !b.hidden && !b.disabled && b.offsetParent !== null);
  let cur = null;
  function rove(to) {
    const list = items();
    cur = to || (list.includes(cur) ? cur : list[0]);
    for (const b of bar.querySelectorAll(".nx-b")) b.tabIndex = b === cur ? 0 : -1;
  }
  bar.addEventListener("focusin", (e) => e.target.classList.contains("nx-b") && rove(e.target));
  function closePop(focus) {
    if (pop.hidden) return;
    pop.hidden = true;
    mb.setAttribute("aria-expanded", "false");
    focus && mb.focus();
  }
  bar.addEventListener("keydown", (e) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.key === "Escape") {
      if (!pop.hidden) {
        e.stopPropagation();
        closePop(true);
      }
      return;
    }
    // Keep arrows/Enter/Space out of the document's page-nav and editor key handlers.
    e.stopPropagation();
    const list = items();
    const i = list.indexOf(root.activeElement);
    const go = { ArrowRight: i + 1, ArrowDown: i + 1, ArrowLeft: i - 1, ArrowUp: i - 1, Home: 0, End: list.length - 1 }[e.key];
    if (go === undefined || i < 0) return;
    e.preventDefault();
    const t = list[(go + list.length) % list.length];
    rove(t);
    t.focus();
  });
  bar.addEventListener("keyup", (e) => e.stopPropagation());

  // Mouse presses must not steal focus from the document surface.
  bar.addEventListener("pointerdown", (e) => e.target.closest(".nx-b") && e.preventDefault());
  bar.addEventListener("click", (e) => {
    const b = e.target.closest(".nx-b");
    if (!b || b.disabled) return;
    if (b === mb) {
      const open = pop.hidden;
      pop.hidden = !open;
      mb.setAttribute("aria-expanded", String(open));
      mb.focus();
      return;
    }
    const it = btns[b.dataset.i];
    if (!pop.hidden) closePop(false);
    if (it.c) host.shellAction("cmd", it.c);
    else host.shellAction(it.x && b.getAttribute("aria-pressed") === "true" ? it.x : it.o, it.a || "");
  });
  const outside = (e) => !e.composedPath().includes(more) && closePop(false);
  document.addEventListener("pointerdown", outside);

  // The Rust chrome is re-rendered with innerHTML on every action: re-sync after each swap.
  const mo = new MutationObserver(sync);
  mo.observe(chrome, { childList: true });
  const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => fit());
  ro && ro.observe(bar);
  sync();

  return {
    destroy() {
      mo.disconnect();
      ro && ro.disconnect();
      document.removeEventListener("pointerdown", outside);
      bar.remove();
      style.remove();
      host.removeAttribute("data-nx");
      host._emb = null;
    },
  };
}
