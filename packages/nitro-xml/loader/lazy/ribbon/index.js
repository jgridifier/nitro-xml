// SPDX-License-Identifier: MIT
// Office-style frame (title bar, tabs, ribbon, panes, status bar) around a Nitro viewer element.
// Loaded by dynamic import() from the demo host; lives under loader/lazy/ so it never counts
// toward the first-load budget. Every button calls the element's existing shellAction(op, arg);
// enabled/pressed/readout state is mirrored from the Rust chrome (host._ch), which stays the
// single source of truth. The frame hides the Rust toolbar/tab strip with injected shadow CSS;
// nothing in the element's default mode changes.
import { applyTheme } from "../theme.js";
import { ICONS } from "./icons.js";
import { CANVAS_CSS, CSS, SHADOW_CSS } from "./style.js";
import { QAT, officeCommandHidden, tabsFor } from "./commands.js";
import { trackDirty } from "./dirty.js";
import { createThumbs } from "./thumbs.js";
import { attachThumbDrag } from "./thumb-drag.js";

const APPS = {
  xlsx: { name: "Excel", tile: "X", unit: "Sheet", noun: "workbook", ext: ".xlsx,.xlsm,.xltx,.xltm" },
  docx: { name: "Word", tile: "W", unit: "Page", noun: "document", ext: ".docx,.docm,.dotx,.dotm" },
  pptx: { name: "PowerPoint", tile: "P", unit: "Slide", noun: "presentation", ext: ".pptx,.pptm,.potx,.potm" },
};
export const APP_INFO = APPS;

const svg = (i) => `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[i] || ""}</svg>`;
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => "&#" + c.charCodeAt(0) + ";");
const plain = (t) => t.replace(/ \(.*/, "");

/** Persisted/system preference when the host passes no theme: `dark` or the familiar `office` skin. */
export function getTheme() {
  try {
    const t = localStorage.getItem("nitro-theme");
    if (t === "dark" || t === "office") return t;
  } catch {}
  return matchMedia && matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "office";
}
export function setTheme(t) {
  document.documentElement.dataset.theme = t === "dark" ? "dark" : "light";
  try {
    localStorage.setItem("nitro-theme", t === "dark" ? "dark" : "office");
  } catch {}
  document.querySelectorAll(".ox").forEach((o) => applyTheme(o, t));
}

/**
 * @param {{ container: HTMLElement, el: HTMLElement, format: 'xlsx'|'docx'|'pptx', chrome?: boolean,
 *   mode: 'full'|'embedded', links?: [string,string][], onMode: (m:string)=>void,
 *   theme?: 'office'|'quiet'|'dark'|Record<string,string> (docs/theme.md; default: saved/system light|dark),
 *   onFile: (f:File)=>void, onSample: ()=>void, onClose?: ()=>void, edit?: boolean }} o
 *   edit: the element is writable and the format's editor is wired to the handle (xlsx today).
 */
export function mountOffice(o) {
  const app = APPS[o.format];
  const full = o.chrome !== false;
  const edit = !!o.edit;
  const tabsSpec = tabsFor(o.format, app.unit, edit);
  const qat = edit && full ? QAT.map((x, i) => ({ ...x, k: "q:" + i })) : [];
  /** @type {Map<string, any>} control by key */
  const byKey = new Map();
  for (const x of qat) byKey.set(x.k, x);
  for (const groups of Object.values(tabsSpec))
    for (const g of groups) for (const x of [...g.big, ...g.rows.flat()]) if (!x.r) [x, ...(x.m || [])].forEach((y) => byKey.set(y.k, y));
  const root = document.createElement("div");
  root.className = "ox";
  root.dataset.app = o.format;
  const base = o.theme ?? getTheme();
  applyTheme(root, base);
  const style = document.createElement("style");
  style.textContent = CSS;

  const lbl = (x) => esc(plain(x.t || x.l));
  const btn = (x, size) =>
    `<button type="button" class="rb ${size}${x.m ? " dd" : ""}${x.wide ? " wide" : ""}" tabindex="-1" aria-label="${lbl(x)}" title="${esc(x.t || x.l)}" data-k="${x.k}"${x.p ? ' aria-pressed="false"' : ""}${x.m ? ' aria-haspopup="menu" aria-expanded="false"' : ""}>${svg(x.i)}${size === "lg" || x.wide ? `<span>${esc(x.l)}</span>` : ""}${x.m ? svg("chevDown") : ""}</button>`;
  const panel = (name) =>
    tabsSpec[name]
      .map(
        (g) =>
          `<div class="rg" role="group" aria-label="${esc(g.g)}"><button type="button" class="rgc" aria-haspopup="true" aria-expanded="false" aria-label="${esc(g.g)}"><span>${esc(g.g)}</span>${svg("chevDown")}</button><div class="rr">${g.big
            .map((x) => (x.r ? `<span class="rd" data-r="${x.r}" role="status"></span>` : btn(x, "lg")))
            .join("")}${g.rows.length ? `<div class="col2">${g.rows.map((r) => `<div class="r1">${r.map((x) => btn(x, "sm")).join("")}</div>`).join("")}</div>` : ""}</div><div class="gl" aria-hidden="true">${esc(g.g)}</div></div>`,
      )
      .join("");
  const links = (o.links || []).map(([h, t]) => `<a href="${esc(h)}">${esc(t)}</a>`).join("");
  const seg = (m, l) => `<button type="button" data-mode="${m}" aria-pressed="${o.mode === m}" title="${l} toolbar mode">${l}</button>`;
  root.innerHTML = `
<header class="tb" role="banner">
  <span class="tile" aria-hidden="true">${app.tile}</span>
  <button type="button" class="ib" data-q="open" aria-label="Open file" title="Open file">${svg("open")}</button>
  <span class="sep" aria-hidden="true"></span>
  ${edit ? `<div class="qat" role="toolbar" aria-label="Quick access">${qat.map((x) => `<button type="button" class="ib" data-k="${x.k}" aria-label="${lbl(x)}" title="${esc(x.t)}">${svg(x.i)}</button>`).join("")}</div><span class="sep" aria-hidden="true"></span>` : ""}
  <div class="doc"><h1 data-doc>${esc(app.name)}</h1><i data-state aria-live="polite"></i></div>
  ${full ? `<span class="srch"><label class="sr" style="position:absolute;width:1px;height:1px;overflow:hidden" for="ox-q">Search in document</label>${svg("find")}<input id="ox-q" type="search" autocomplete="off" placeholder="Search this ${app.noun}" aria-label="Search in document" /></span>` : `<span class="grow"></span>`}
  <div class="seg" role="group" aria-label="Toolbar mode">${seg("full", "Full")}${seg("embedded", "Embedded")}</div>
  <span class="lnks">${links}</span>
  <button type="button" class="ib" data-q="theme" aria-label="Toggle dark theme" title="Toggle light / dark theme">${svg("moon")}</button>
</header>
${
  full
    ? `<div role="region" aria-label="Ribbon"><div class="tabs"><button type="button" class="file" aria-haspopup="menu" aria-expanded="false">File</button>
<div role="tablist" aria-label="Ribbon tabs" style="display:flex">${Object.keys(tabsSpec)
        .map((n, i) => `<button type="button" role="tab" id="ox-t-${n}" aria-controls="ox-p" aria-selected="${i === 0}" tabindex="${i ? -1 : 0}" data-t="${n}">${n}</button>`)
        .join("")}</div><span class="grow"></span><button type="button" class="col" aria-label="Collapse the ribbon" aria-expanded="true" title="Collapse the ribbon">${svg("chevUp")}</button></div>
<div class="fm" role="menu" hidden aria-label="File">
  <button type="button" role="menuitem" data-f="open">${svg("open")}Open…<small>from your device</small></button>
  <button type="button" role="menuitem" data-f="sample">${svg("sample")}Open sample ${app.noun}</button>
  ${edit ? `<button type="button" role="menuitem" data-f="save">${svg("save")}Save a copy<small>download</small></button>` : ""}
  <button type="button" role="menuitem" data-f="close">${svg("close")}Close</button>
</div>
<div class="dm" role="menu" hidden></div>
<div class="rbn" id="ox-p" role="tabpanel" aria-labelledby="ox-t-Home"><div class="rbn-in" style="display:contents"></div></div></div>`
    : ""
}
<div class="body" role="main" aria-label="${esc(app.name)} document">
  ${o.format === "pptx" && full ? `<aside class="pane" aria-label="Slide thumbnails" hidden><h2>Slides</h2><div class="pl" role="list"></div></aside>` : ""}
  <div class="stage"><div class="over" hidden></div></div>
</div>
<div class="foot" role="region" aria-label="Status">
${o.format === "xlsx" && full ? `<div class="sheets" role="tablist" aria-label="Sheets" hidden></div>` : ""}
${
  full
    ? `<div class="sb"><span data-pos></span><span data-find></span><span class="grow"></span>
<span class="zr"><button type="button" class="ib" data-z="zoom-out" aria-label="Zoom out" title="Zoom out">${svg("zoomOut")}</button>
<input type="range" min="10" max="400" step="10" value="100" aria-label="Zoom" />
<button type="button" class="ib" data-z="zoom-in" aria-label="Zoom in" title="Zoom in">${svg("zoomIn")}</button>
<button type="button" class="zl" data-z="reset" aria-label="Reset zoom to 100%" title="Reset zoom to 100%">100%</button></span></div>`
    : ""
}
</div>`;
  o.container.replaceChildren(style, root);
  const $ = (s) => root.querySelector(s);
  const $$ = (s) => [...root.querySelectorAll(s)];
  const stage = $(".stage");
  stage.prepend(o.el);

  const host = () => o.el;
  const act = (op, arg) => host().shellAction && host().shellAction(op, arg || "");

  // ---- injected rules for the viewer's shadow root ----
  let shadowStyle = null;
  function skin() {
    const sr = o.el.shadowRoot;
    if (!sr || !full || shadowStyle?.isConnected) return;
    shadowStyle = document.createElement("style");
    shadowStyle.textContent = SHADOW_CSS + (o.format === "docx" ? CANVAS_CSS : "");
    sr.appendChild(shadowStyle);
  }

  const thumbs = o.format === "pptx" && full ? createThumbs() : null;
  let io = null;

  // ---- state mirrored from the Rust chrome ----
  const ch = () => o.el._ch;
  const q = (s) => ch() && ch().querySelector(s);
  let page = { at: 0, n: 0 };
  function sync() {
    skin();
    if (!full) return;
    const tb = ch() ? [...ch().querySelectorAll(".t button")] : [];
    const at = tb.findIndex((b) => b.getAttribute("aria-current") === "true");
    page = { at, n: tb.length };
    const findOpen = !!q(".f") && !q(".f").hidden;
    const diagOpen = !!q(".d") && !q(".d").hidden;
    const zoomTxt = (q(".z") || {}).textContent || "100%";
    const zoom = parseInt(zoomTxt, 10) || 100;
    for (const b of [...$$(".rbn .rb"), ...$$(".qat .ib")]) {
      const it = byKey.get(b.dataset.k);
      let dis = !ch();
      let on = false;
      if (it.n) {
        b.hidden = tb.length < 2;
        dis = at < 0 || at + it.n < 0 || at + it.n >= tb.length;
      } else if (it.c) {
        // Mirror the Rust ribbon's own enabled state where it has a control for the command.
        // PowerPoint and Word hide a button the strip does not emit, so a dead command
        // cannot stay clickable after the editor drops it.
        const m = q(`[data-a="cmd"][data-g="${it.c}"]`);
        dis = dis || !!(m && m.disabled) || (it.c === "sheet.delete" && tb.length < 2);
        b.hidden = officeCommandHidden(o.format, !!ch(), !!m);
      } else if (it.x) on = findOpen;
      else if (it.d) on = diagOpen;
      else if (it.s) on = !$(".pane").dataset.off;
      b.disabled = dis;
      if (it.p) b.setAttribute("aria-pressed", String(on));
    }
    for (const r of $$(".rd")) {
      r.textContent = r.dataset.r === "zoom" ? zoomTxt : tb.length > 1 ? `${at + 1} of ${tb.length}` : "";
      r.hidden = !r.textContent;
    }
    for (const g of $$(".rg")) g.hidden = !g.querySelector(".rb:not([hidden]), .rd:not([hidden])");
    $("[data-pos]").textContent = tb.length ? `${app.unit} ${at + 1} of ${tb.length}` : "";
    $("[data-find]").textContent = findOpen ? (q(".fs") || {}).textContent || "" : "";
    $(".sb input").value = zoom;
    $(".sb .zl").textContent = zoomTxt;
    const pane = $(".pl");
    if (pane) {
      const sig = tb.map((b) => b.dataset.g + b.textContent).join("|");
      if (pane.dataset.sig !== sig) {
        pane.dataset.sig = sig;
        pane.innerHTML = tb
          .map((b, i) => `<button type="button" role="listitem" data-g="${b.dataset.g}" aria-label="${app.unit} ${i + 1}: ${esc(b.textContent)}"><span class="n" aria-hidden="true">${i + 1}</span><span class="th"><span class="ph">${esc(b.textContent)}</span></span></button>`)
          .join("");
        lazyThumbs();
      }
      [...pane.children].forEach((b, i) => (i === at ? b.setAttribute("aria-current", "true") : b.removeAttribute("aria-current")));
      pane.closest(".pane").hidden = tb.length < 2 || !!pane.closest(".pane").dataset.off;
    }
    const sheets = $(".sheets");
    if (sheets) {
      sheets.innerHTML = tb.map((b, i) => `<button type="button" role="tab" data-g="${b.dataset.g}" aria-selected="${i === at}"${i === at ? ' aria-current="true"' : ""}>${esc(b.textContent)}</button>`).join("");
      sheets.hidden = !tb.length;
    }
  }
  const mo = new MutationObserver(sync);
  let watched = null;
  const watch = () => {
    if (ch() && watched !== ch()) {
      watched = ch();
      mo.observe(watched, { childList: true });
    }
    sync();
  };
  const poll = setInterval(watch, 400);
  watch();

  // ---- events ----
  root.addEventListener("click", (e) => {
    const t = e.target.closest("button");
    if (!t || !root.contains(t) || t.disabled || t.hidden) return;
    if (t.classList.contains("rgc")) return group(t.parentElement, !t.parentElement.dataset.open);
    if (t.dataset.mode) return o.onMode(t.dataset.mode);
    if (t.dataset.q === "open") return pick.click();
    if (t.dataset.q === "theme") {
      const dark = root.dataset.theme !== "dark";
      const next = dark ? "dark" : base === "dark" ? "office" : base;
      if (o.theme === undefined) return setTheme(next);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
      return void applyTheme(root, next);
    }
    if (t.closest(".dm")) {
      const m = byKey.get(t.dataset.k);
      dropdown(false, true);
      return void (m && act("cmd", m.c));
    }
    if (t.dataset.k) {
      const it = byKey.get(t.dataset.k);
      if (it.m) return dropdown(dm.hidden || dm._for !== t, false, t, it);
      if (it.s) return paneToggle();
      if (it.c) return void (group(null), act("cmd", it.c));
      return void (group(null), act(it.x && t.getAttribute("aria-pressed") === "true" ? it.x : it.o, it.a));
    }
    if (t.closest(".pane, .sheets")) return void act("sheet", t.dataset.g);
    if (t.dataset.z) {
      return void act(t.dataset.z === "reset" ? "zoom-set" : t.dataset.z, t.dataset.z === "reset" ? "100" : "");
    }
    if (t.dataset.t) return selectTab(t.dataset.t, true);
    if (t.classList.contains("col")) return collapse();
    if (t.classList.contains("file")) return menu(menuEl.hidden);
    if (t.dataset.f) {
      menu(false);
      if (t.dataset.f === "open") pick.click();
      else if (t.dataset.f === "sample") o.onSample();
      else if (t.dataset.f === "save") act("cmd", "file.save");
      else o.onClose && o.onClose();
    }
  });
  const menuEl = $(".fm");
  function menu(open) {
    if (!menuEl) return;
    const f = $(".file");
    menuEl.hidden = !open;
    f.setAttribute("aria-expanded", String(open));
    if (open) {
      const r = f.getBoundingClientRect();
      const b = root.getBoundingClientRect();
      menuEl.style.left = r.left - b.left + "px";
      menuEl.style.top = r.bottom - b.top + "px";
      menuEl.querySelector("button").focus();
    }
  }
  document.addEventListener("pointerdown", (e) => menuEl && !menuEl.hidden && !e.composedPath().some((n) => n === menuEl || n === $(".file")) && menu(false));
  // Dropdown (number format): one popup, filled from the control's own entries.
  const dm = $(".dm");
  function dropdown(open, focus, btn, it) {
    if (!dm) return;
    if (!open) {
      const b = dm._for;
      dm.hidden = true;
      dm._for = null;
      b && b.setAttribute("aria-expanded", "false");
      return void (focus && b && b.focus());
    }
    dm.innerHTML = it.m.map((m) => `<button type="button" role="menuitem" tabindex="-1" data-k="${m.k}">${esc(m.l)}</button>`).join("");
    dm.setAttribute("aria-label", it.l);
    const r = btn.getBoundingClientRect();
    const b = root.getBoundingClientRect();
    dm.style.left = r.left - b.left + "px";
    dm.style.top = r.bottom - b.top + "px";
    dm.hidden = false;
    dm._for = btn;
    btn.setAttribute("aria-expanded", "true");
    dm.querySelector("button").focus();
  }
  document.addEventListener("pointerdown", (e) => dm && !dm.hidden && !e.composedPath().some((n) => n === dm || n === dm._for) && dropdown(false));
  // Narrow frames (CSS container query) collapse each ribbon group to a chip; its buttons open as a popup.
  function group(g, open) {
    for (const x of $$(".rg[data-open]")) {
      if (x === g && open) continue;
      delete x.dataset.open;
      x.querySelector(".rgc").setAttribute("aria-expanded", "false");
      x.querySelector(".rr").style.cssText = "";
    }
    if (!g || !open) return;
    const r = g.querySelector(".rgc").getBoundingClientRect();
    const pop = g.querySelector(".rr");
    g.dataset.open = "1";
    g.querySelector(".rgc").setAttribute("aria-expanded", "true");
    pop.style.cssText = `top:${r.bottom}px;left:${Math.max(4, Math.min(r.left, innerWidth - 4 - pop.offsetWidth))}px`;
    const first = [...pop.querySelectorAll(".rb")].find((b) => !b.disabled && !b.hidden);
    first && first.focus();
  }
  document.addEventListener("pointerdown", (e) => $(".rg[data-open]") && !e.composedPath().some((n) => n.classList?.contains("rg")) && group(null));
  function paneToggle() {
    const pane = $(".pane");
    if (!pane) return;
    if (pane.dataset.off) delete pane.dataset.off;
    else pane.dataset.off = "1";
    sync();
  }
  function selectTab(name, focus) {
    dropdown(false);
    for (const t of $$("[role=tab][data-t]")) {
      const on = t.dataset.t === name;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      if (on && focus) t.focus();
    }
    const p = $("#ox-p");
    p.setAttribute("aria-labelledby", "ox-t-" + name);
    $(".rbn-in").innerHTML = panel(name) + (edit || name !== "Home" ? "" : `<div class="rn" role="note">${svg("info")}<span><b>View only.</b> ${app.name} editing isn’t connected to the browser engine yet. <a href="./xlsx-full.html">Try the Excel demo</a> to edit.</span></div>`);
    p.hidden = false;
    $(".col").setAttribute("aria-expanded", "true");
    rove();
    sync();
  }
  function collapse() {
    const p = $("#ox-p");
    p.hidden = !p.hidden;
    $(".col").setAttribute("aria-expanded", String(!p.hidden));
    $(".col").setAttribute("aria-label", p.hidden ? "Expand the ribbon" : "Collapse the ribbon");
  }
  root.addEventListener("keydown", (e) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const t = e.target;
    if (dm && !dm.hidden && (t.closest(".dm") || t === dm._for)) {
      const l = [...dm.querySelectorAll("button")];
      if (e.key === "Escape") {
        e.preventDefault();
        return dropdown(false, true);
      }
      if (e.key === "Tab") return dropdown(false);
      if (["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) {
        e.preventDefault();
        const i = l.indexOf(t);
        l[e.key === "Home" ? 0 : e.key === "End" ? l.length - 1 : (i + (e.key === "ArrowDown" ? 1 : -1) + l.length) % l.length].focus();
      }
      return;
    }
    if (e.key === "ArrowDown" && t.matches?.(".rb.dd")) {
      e.preventDefault();
      return t.click();
    }
    if (e.key === "Escape" && $(".rg[data-open]")) {
      const c = $(".rg[data-open] .rgc");
      group(null);
      return c.focus();
    }
    if (e.key === "Escape" && menuEl && !menuEl.hidden) {
      menu(false);
      return $(".file").focus();
    }
    const stepIn = (list, i, d) => {
      e.preventDefault();
      const n = list[(i + d + list.length) % list.length];
      n.focus();
      return n;
    };
    if (t.closest(".fm")) {
      const l = [...menuEl.querySelectorAll("button")];
      if (e.key === "ArrowDown" || e.key === "ArrowUp") stepIn(l, l.indexOf(t), e.key === "ArrowDown" ? 1 : -1);
      return;
    }
    if (t.matches("[role=tab][data-t]") && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
      const l = $$("[role=tab][data-t]");
      e.preventDefault();
      selectTab(l[(l.indexOf(t) + (e.key === "ArrowRight" ? 1 : -1) + l.length) % l.length].dataset.t, true);
    } else if (t.closest(".sheets") && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
      const l = $$(".sheets button");
      stepIn(l, l.indexOf(t), e.key === "ArrowRight" ? 1 : -1);
    } else if (t.closest(".rbn") && ["ArrowRight", "ArrowLeft", "Home", "End"].includes(e.key)) {
      const l = $$(".rbn .rb").filter((b) => !b.hidden && !b.disabled && b.offsetParent);
      const i = l.indexOf(t);
      if (i < 0) return;
      e.preventDefault();
      const n = l[e.key === "Home" ? 0 : e.key === "End" ? l.length - 1 : (i + (e.key === "ArrowRight" ? 1 : -1) + l.length) % l.length];
      cur = n;
      rove();
      n.focus();
    }
  });
  let cur = null;
  function rove() {
    const l = $$(".rbn .rb").filter((b) => !b.hidden && !b.disabled && b.offsetParent);
    if (!l.includes(cur)) cur = l[0];
    for (const b of $$(".rbn .rb")) b.tabIndex = b === cur ? 0 : -1;
  }
  root.addEventListener("focusin", (e) => e.target.classList?.contains("rb") && ((cur = e.target), rove()));
  root.addEventListener("pointerdown", (e) => e.target.closest(".rb, .dm button, .pane button, .sheets button") && e.preventDefault());
  const rv = new MutationObserver(rove);
  typeof ResizeObserver !== "undefined" && new ResizeObserver(() => (group(null), rove())).observe(root);
  full && rv.observe($(".rbn-in"), { subtree: true, attributes: true, childList: true, attributeFilter: ["disabled", "hidden"] });

  // search box → the same find-open / find-query / find-next ops the Rust bar uses
  const si = $(".srch input");
  if (si) {
    let opened = false;
    si.addEventListener("input", async () => {
      if (!opened) {
        opened = true;
        await act("find-open");
      }
      act("find-query", si.value);
    });
    si.addEventListener("keydown", (e) => {
      if (e.key === "Enter") act(e.shiftKey ? "find-prev" : "find-next");
      else if (e.key === "Escape") {
        si.value = "";
        opened = false;
        act("find-close");
      }
      e.stopPropagation();
    });
    si.addEventListener("keyup", (e) => e.stopPropagation());
  }
  let zt = 0;
  let pendingZoom = null;
  const zr = $(".sb input");
  // sync() copies the chrome readout back onto this range on a timer. Reading
  // `.value` when the debounce fires can undo a step that has not reached
  // zoom-set yet. Chromium headless also skips the native range step on arrow
  // keys, so the key handler applies the step and the queued percent is the
  // one that gets sent.
  const queueZoom = (value) => {
    pendingZoom = String(value);
    clearTimeout(zt);
    zt = setTimeout(() => {
      const v = pendingZoom;
      pendingZoom = null;
      if (v != null) act("zoom-set", v);
    }, 60);
  };
  if (zr) {
    zr.addEventListener("input", () => queueZoom(zr.value));
    zr.addEventListener("change", () => {
      if (pendingZoom == null) queueZoom(zr.value);
    });
    zr.addEventListener("keydown", (e) => {
      const step = Number(zr.step) || 10;
      const min = Number(zr.min) || 0;
      const max = Number(zr.max) || 100;
      let next = Number(pendingZoom != null ? pendingZoom : zr.value);
      if (e.key === "ArrowRight" || e.key === "ArrowUp") next += step;
      else if (e.key === "ArrowLeft" || e.key === "ArrowDown") next -= step;
      else if (e.key === "PageUp") next += step * 10;
      else if (e.key === "PageDown") next -= step * 10;
      else if (e.key === "Home") next = min;
      else if (e.key === "End") next = max;
      else return;
      e.preventDefault();
      next = Math.min(max, Math.max(min, next));
      zr.value = String(next);
      queueZoom(next);
    });
  }

  // ---- slide thumbnails (pptx pane): rendered from the real slides, only when scrolled into view ----
  function lazyThumbs() {
    if (!thumbs || typeof IntersectionObserver === "undefined") return;
    io && io.disconnect();
    const pl = $(".pl");
    io = new IntersectionObserver(
      (hits) => {
        for (const h of hits) {
          if (!h.isIntersecting) continue;
          const box = h.target.querySelector(".th");
          io.unobserve(h.target);
          if (!box || box.dataset.live) continue;
          thumbs.render(h.target.dataset.g).then((r) => box.isConnected && thumbs.mount(box, r));
        }
      },
      { root: pl.closest(".pane"), rootMargin: "160px 0px" },
    );
    for (const b of pl.children) io.observe(b);
  }
  // Reorder: apply the edit, follow the moved slide, and re-seed the thumbnail renderer.
  let docName = "";
  async function moveSlide(from, to) {
    const d = o.el._d;
    if (!d || !edit) return;
    await d.applyEdit(JSON.stringify({ command: "insert.move-slide-to", from, to }));
    await o.el.shellAction("sheet", String(to));
    track && track.touch();
    thumbs.reorder(from, to, await d.save(), docName);
    const pl = $(".pl");
    if (pl) pl.dataset.sig = "";
    sync();
  }
  thumbs && $(".pl") && attachThumbDrag($(".pl"), { enabled: () => edit && !!o.el._d, move: (a, b) => void moveSlide(a, b).catch((e) => console.error("moveSlide", e && e.message || e)) });
  const thumbSize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => thumbs && $$(".pl .th[data-live]").forEach((b) => thumbs.fit(b)));
  thumbs && $(".pl") && thumbSize && thumbSize.observe($(".pl"));

  // ---- unsaved changes (editable workbooks) ----
  let state = "";
  let dirty = false;
  const setState = (text, kind) => {
    const i = $("[data-state]");
    if (!i) return;
    i.textContent = text || "";
    i.dataset.s = kind || "";
    i.title = kind === "ro" ? "This format is view-only in the browser build: its editor is not wired to the WASM handle yet." : "";
  };
  const warn = (e) => dirty && (e.preventDefault(), (e.returnValue = ""));
  let track = null;
  if (edit) {
    track = trackDirty(o.el, (d) => {
      dirty = d;
      root.toggleAttribute("data-dirty", d);
      setState(d ? "Unsaved changes" : state === "saved" ? "Saved" : "No changes", d ? "dirty" : "clean");
    });
    o.el.addEventListener("nitro-save", (e) => {
      state = "saved";
      track.saved(e.detail);
    });
    addEventListener("beforeunload", warn);
    // The engine's chrome and the typed-cell path both mutate behind the element: re-check after an action.
    const shell = o.el.shellAction.bind(o.el);
    o.el.shellAction = async (op, arg) => {
      const r = await shell(op, arg);
      if (op === "cmd" || op.startsWith("formula-")) track.touch();
      return r;
    };
  }

  // ---- file picker + drag and drop ----
  const pick = document.createElement("input");
  pick.type = "file";
  pick.accept = app.ext;
  pick.hidden = true;
  pick.addEventListener("change", () => {
    const f = pick.files && pick.files[0];
    pick.value = "";
    f && accept(f);
  });
  root.append(pick);
  const okExt = (n) => app.ext.split(",").some((x) => n.toLowerCase().endsWith(x));
  function accept(f) {
    if (!okExt(f.name)) return ui.error(`“${f.name}” isn’t a ${app.ext.split(",")[0]} file. Choose a ${app.name} ${app.noun} to open.`);
    o.onFile(f);
  }
  let depth = 0;
  let wasHidden = true;
  const over = $(".over");
  const hasFiles = (e) => e.dataTransfer && [...e.dataTransfer.types].includes("Files");
  root.addEventListener("dragenter", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    if (!depth++) {
      wasHidden = over.hidden;
      drop("drag");
    }
    over.dataset.drag = "1";
  });
  root.addEventListener("dragover", (e) => hasFiles(e) && e.preventDefault());
  root.addEventListener("dragleave", (e) => {
    if (!hasFiles(e) || --depth > 0) return;
    depth = 0;
    delete over.dataset.drag;
    if (wasHidden) over.hidden = true;
    else drop("empty");
  });
  root.addEventListener("drop", (e) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    depth = 0;
    delete over.dataset.drag;
    const f = e.dataTransfer.files[0];
    if (wasHidden) over.hidden = true;
    f && accept(f);
  });

  // ---- empty / loading / error states ----
  function drop(kind, text) {
    over.hidden = false;
    // Lines paint under this veil, before ready(). While it hit-tests, the first
    // click of a cold WebKit load lands on "Opening…" and the caret never appears.
    over.style.pointerEvents = kind === "loading" ? "none" : "";
    const ext = app.ext.split(",")[0];
    if (kind === "loading") {
      over.innerHTML = `<div class="drop" role="status"><div class="spin" aria-hidden="true"></div><h1>${esc(text || "Opening…")}</h1><p>Everything happens in your browser.</p></div>`;
      return;
    }
    over.innerHTML = `<div class="drop"><span class="tile" aria-hidden="true">${app.tile}</span>
<h1>${kind === "drag" ? `Drop to open` : `Drop a ${ext} here or click to browse`}</h1>
<p>The file is processed entirely in your browser. Nothing is uploaded.</p>
<p class="err" role="alert"${kind === "error" ? "" : " hidden"}>${esc(text || "")}</p>
<div class="row"><button type="button" class="btn pri" data-d="browse">Browse files</button><button type="button" class="btn" data-d="sample">Open the sample ${app.noun}</button></div></div>`;
    kind === "error" && over.querySelector(".err").scrollIntoView?.({ block: "nearest" });
  }
  over.addEventListener("click", (e) => {
    const b = e.target.closest("[data-d]");
    if (!b) return;
    b.dataset.d === "browse" ? pick.click() : o.onSample();
  });

  const ui = {
    root,
    el: o.el,
    doc(name, status) {
      $("[data-doc]").textContent = name || app.name;
      if (edit && name) {
        state = "";
        dirty = false;
        setState("No changes", "clean");
        track.rebase();
      } else setState(status, status === "Read-only" ? "ro" : "");
    },
    /** Hand the opened file to the thumbnail renderer (pptx pane). */
    opened(bytes, name) {
      if (!thumbs) return;
      docName = name;
      thumbs.start(bytes, name);
      const pl = $(".pl");
      if (pl) {
        pl.dataset.sig = "";
        sync();
      }
    },
    get dirty() {
      return dirty;
    },
    /** Serialise the current (possibly edited) workbook; used to carry edits across a mode switch. */
    async snapshot() {
      return edit && o.el._d ? o.el._d.save() : null;
    },
    /** Re-flag unsaved edits after a reopen (the reopened bytes are the edited ones, so a compare would read clean). */
    markDirty() {
      track && track.force();
    },
    loading: (t) => drop("loading", t),
    empty: () => drop("empty"),
    error: (m) => drop("error", m),
    ready: () => (over.hidden = true),
    destroy() {
      clearInterval(poll);
      track && track.destroy();
      io && io.disconnect();
      thumbSize && thumbSize.disconnect();
      thumbs && thumbs.stop();
      removeEventListener("beforeunload", warn);
      mo.disconnect();
      rv.disconnect();
      root.remove();
      style.remove();
    },
  };
  if (full) selectTab("Home");
  return ui;
}
