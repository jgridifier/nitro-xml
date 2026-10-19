// SPDX-License-Identifier: MIT
// Office-style frame (title bar, tabs, ribbon, panes, status bar) around a Nitro viewer element.
// Loaded by dynamic import() from the demo host; lives under loader/lazy/ so it never counts
// toward the first-load budget. Every button calls the element's existing shellAction(op, arg);
// enabled/pressed/readout state is mirrored from the Rust chrome (host._ch), which stays the
// single source of truth. The frame hides the Rust toolbar/tab strip with injected shadow CSS;
// nothing in the element's default mode changes.
import { ICONS } from "./icons.js";
import { CANVAS_CSS, CSS, SHADOW_CSS } from "./style.js";

const APPS = {
  xlsx: { name: "Excel", tile: "X", unit: "Sheet", noun: "workbook", ext: ".xlsx,.xlsm,.xltx,.xltm" },
  docx: { name: "Word", tile: "W", unit: "Page", noun: "document", ext: ".docx,.docm,.dotx,.dotm" },
  pptx: { name: "PowerPoint", tile: "P", unit: "Slide", noun: "presentation", ext: ".pptx,.pptm,.potx,.potm" },
};
export const APP_INFO = APPS;

const svg = (i) => `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${ICONS[i] || ""}</svg>`;
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => "&#" + c.charCodeAt(0) + ";");

export function getTheme() {
  try {
    const t = localStorage.getItem("nitro-theme");
    if (t) return t;
  } catch {}
  return matchMedia && matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}
export function setTheme(t) {
  document.documentElement.dataset.theme = t;
  try {
    localStorage.setItem("nitro-theme", t);
  } catch {}
  document.querySelectorAll(".ox").forEach((o) => (o.dataset.theme = t));
}

// Ribbon content. Only commands that exist on the element's shellAction are listed.
// k key · i icon · l label · o op · a op arg · x op sent while pressed · p toggle · n tab step · r readout
const nav = (unit) => ({
  g: unit + "s",
  items: [
    { i: "prev", l: "Previous", t: `Previous ${unit.toLowerCase()}`, o: "nav-prev", n: -1 },
    { r: "nav" },
    { i: "next", l: "Next", t: `Next ${unit.toLowerCase()}`, o: "nav-next", n: 1 },
  ],
});
const TABS = (unit) => ({
  Home: [
    { g: "Clipboard", items: [{ i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy" }] },
    nav(unit),
    { g: "Editing", items: [{ i: "find", l: "Find", t: "Find (Ctrl+F)", o: "find-open", x: "find-close", p: 1 }] },
  ],
  View: [
    {
      g: "Zoom",
      items: [
        { i: "zoomOut", l: "Zoom out", t: "Zoom out (Ctrl+-)", o: "zoom-out" },
        { r: "zoom" },
        { i: "zoomIn", l: "Zoom in", t: "Zoom in (Ctrl++)", o: "zoom-in" },
        { i: "zoom100", l: "100%", t: "Zoom to 100%", o: "zoom-set", a: "100" },
      ],
    },
    { g: "Show", items: [{ i: "diag", l: "Diagnostics", t: "Show or hide load diagnostics", o: "diagnostics-toggle", p: 1, d: 1 }] },
  ],
});

/**
 * @param {{ container: HTMLElement, el: HTMLElement, format: 'xlsx'|'docx'|'pptx', chrome?: boolean,
 *   mode: 'full'|'embedded', links?: [string,string][], onMode: (m:string)=>void,
 *   onFile: (f:File)=>void, onSample: ()=>void, onClose?: ()=>void }} o
 */
export function mountOffice(o) {
  const app = APPS[o.format];
  const full = o.chrome !== false;
  const tabsSpec = TABS(app.unit);
  const root = document.createElement("div");
  root.className = "ox";
  root.dataset.app = o.format;
  root.dataset.theme = getTheme();
  const style = document.createElement("style");
  style.textContent = CSS;

  const btn = (it, k) =>
    `<button type="button" class="rb lg" tabindex="-1" aria-label="${esc((it.t || it.l).replace(/ \(.*/, ""))}" title="${esc(it.t || it.l)}" data-k="${k}"${it.p ? ' aria-pressed="false"' : ""}>${svg(it.i)}<span>${esc(it.l)}</span></button>`;
  const panel = (name) => {
    let k = 0;
    return tabsSpec[name]
      .map(
        (g) =>
          `<div class="rg" role="group" aria-label="${esc(g.g)}"><div class="rr">${g.items
            .map((it) => (it.r ? `<span class="rd" data-r="${it.r}" role="status"></span>` : btn(it, name + ":" + k++)))
            .join("")}</div><div class="gl" aria-hidden="true">${esc(g.g)}</div></div>`,
      )
      .join("");
  };
  const links = (o.links || []).map(([h, t]) => `<a href="${esc(h)}">${esc(t)}</a>`).join("");
  const seg = (m, l) => `<button type="button" data-mode="${m}" aria-pressed="${o.mode === m}" title="${l} toolbar mode">${l}</button>`;
  root.innerHTML = `
<header class="tb" role="banner">
  <span class="tile" aria-hidden="true">${app.tile}</span>
  <button type="button" class="ib" data-q="open" aria-label="Open file" title="Open file">${svg("open")}</button>
  <span class="sep" aria-hidden="true"></span>
  <div class="doc"><h1 data-doc>${esc(app.name)}</h1><i data-state></i></div>
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
  <button type="button" role="menuitem" data-f="close">${svg("close")}Close</button>
</div>
<div class="rbn" id="ox-p" role="tabpanel" aria-labelledby="ox-t-Home"><div class="rbn-in" style="display:contents"></div></div></div>`
    : ""
}
<div class="body" role="main" aria-label="${esc(app.name)} document">
  ${o.format === "pptx" && full ? `<aside class="pane" aria-label="Slide thumbnails" hidden><h2>Slides</h2><div class="pl"></div></aside>` : ""}
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
    for (const b of $$(".rbn .rb")) {
      const [tab, k] = b.dataset.k.split(":");
      const it = tabsSpec[tab].flatMap((g) => g.items).filter((x) => !x.r)[+k];
      let dis = !ch();
      let on = false;
      if (it.n) {
        b.hidden = tb.length < 2;
        dis = at < 0 || at + it.n < 0 || at + it.n >= tb.length;
      } else if (it.x) on = findOpen;
      else if (it.d) on = diagOpen;
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
      const sig = tb.map((b) => b.textContent + (b.getAttribute("aria-current") === "true")).join("|");
      if (pane.dataset.sig !== sig) {
        pane.dataset.sig = sig;
        pane.innerHTML = tb
          .map((b, i) => `<button type="button" data-g="${b.dataset.g}"${i === at ? ' aria-current="true"' : ""}><span class="n" aria-hidden="true">${i + 1}</span><span class="th">${esc(b.textContent)}</span></button>`)
          .join("");
      }
      pane.closest(".pane").hidden = tb.length < 2;
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
    if (!t || !root.contains(t) || t.disabled) return;
    if (t.dataset.mode) return o.onMode(t.dataset.mode);
    if (t.dataset.q === "open") return pick.click();
    if (t.dataset.q === "theme") return setTheme(root.dataset.theme === "dark" ? "light" : "dark");
    if (t.dataset.k) {
      const [tab, k] = t.dataset.k.split(":");
      const it = tabsSpec[tab].flatMap((g) => g.items).filter((x) => !x.r)[+k];
      return void act(it.x && t.getAttribute("aria-pressed") === "true" ? it.x : it.o, it.a);
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
  function selectTab(name, focus) {
    for (const t of $$("[role=tab][data-t]")) {
      const on = t.dataset.t === name;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      if (on && focus) t.focus();
    }
    const p = $("#ox-p");
    p.setAttribute("aria-labelledby", "ox-t-" + name);
    $(".rbn-in").innerHTML = panel(name);
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
      const l = $$(".rbn .rb").filter((b) => !b.hidden && !b.disabled);
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
    const l = $$(".rbn .rb").filter((b) => !b.hidden && !b.disabled);
    if (!l.includes(cur)) cur = l[0];
    for (const b of $$(".rbn .rb")) b.tabIndex = b === cur ? 0 : -1;
  }
  root.addEventListener("focusin", (e) => e.target.classList?.contains("rb") && ((cur = e.target), rove()));
  root.addEventListener("pointerdown", (e) => e.target.closest(".rb, .pane button, .sheets button") && e.preventDefault());
  const rv = new MutationObserver(rove);
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
  const zr = $(".sb input");
  zr && zr.addEventListener("input", () => {
    clearTimeout(zt);
    zt = setTimeout(() => act("zoom-set", zr.value), 60);
  });

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
    doc(name, state) {
      $("[data-doc]").textContent = name || app.name;
      $("[data-state]").textContent = state || "";
    },
    loading: (t) => drop("loading", t),
    empty: () => drop("empty"),
    error: (m) => drop("error", m),
    ready: () => (over.hidden = true),
    destroy() {
      clearInterval(poll);
      mo.disconnect();
      rv.disconnect();
      root.remove();
      style.remove();
    },
  };
  if (full) selectTab("Home");
  return ui;
}
