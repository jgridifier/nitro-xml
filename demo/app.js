// SPDX-License-Identifier: MIT
/** Shared demo host: format + mode, sample load, local file picker, mode toggle. */

const TAG = { xlsx: "nitro-xlsx", docx: "nitro-docx", pptx: "nitro-pptx" };
const DEFINE = {
  xlsx: "defineNitroXlsx",
  docx: "defineNitroDocx",
  pptx: "defineNitroPptx",
};
const SAMPLE = {
  xlsx: "./samples/sample.xlsx",
  docx: "./samples/sample.docx",
  pptx: "./samples/sample.pptx",
};

function qs(sel) {
  return document.querySelector(sel);
}

function setError(msg) {
  const el = qs("#err");
  if (!el) return;
  el.textContent = msg || "";
  el.dataset.show = msg ? "1" : "0";
}

async function injectXlsxGridCss(host) {
  if (!host?.shadowRoot) return;
  const { GRID_CSS } = await import("./xlsx-grid-css.js");
  let style = host.shadowRoot.querySelector('style[data-demo="grid"]');
  if (!style) {
    style = document.createElement("style");
    style.dataset.demo = "grid";
    host.shadowRoot.appendChild(style);
  }
  style.textContent = GRID_CSS;
}

/**
 * @param {{ format: 'xlsx'|'docx'|'pptx', mode: 'full'|'embedded' }} opts
 */
export async function boot(opts) {
  const format = opts.format;
  const initialMode = opts.mode === "embedded" ? "embedded" : "full";
  const stage = qs("#stage");
  const title = qs("#title");
  if (title) {
    title.textContent = `${format.toUpperCase()} · ${initialMode === "embedded" ? "embedded" : "normal"} mode`;
  }

  document.querySelectorAll("[data-mode-btn]").forEach((btn) => {
    btn.setAttribute("aria-pressed", btn.dataset.modeBtn === initialMode ? "true" : "false");
  });

  const loader = await import("./nitro-xml/loader/index.js");
  await loader.initCore({ fontSource: false });
  loader[DEFINE[format]]();

  const tag = TAG[format];
  let el = document.createElement(tag);
  el.setAttribute("readonly", "");
  el.setAttribute("mode", initialMode);
  el.style.width = "100%";
  el.style.height = "100%";
  stage.replaceChildren(el);

  /** @type {ArrayBuffer | null} */
  let lastBytes = null;
  let lastName = SAMPLE[format].split("/").pop();

  function waitFrames(n = 2) {
    return new Promise((resolve) => {
      let left = n;
      const step = () => {
        left -= 1;
        if (left <= 0) resolve();
        else requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    });
  }

  async function settlePaint() {
    if (format === "xlsx") await injectXlsxGridCss(el);
    // Match visual harness: push Resize then tick so print/slide layout paints.
    const d = el.document;
    if (d) {
      const dpr = window.devicePixelRatio || 1;
      const w = el.clientWidth || stage.clientWidth || 800;
      const h = el.clientHeight || stage.clientHeight || 600;
      d.pushEvent({ kind: 6, time_ms: performance.now(), width: w, height: h, dpr });
    }
    for (let i = 0; i < 24; i++) {
      if (typeof el._tick === "function") el._tick();
      await waitFrames(1);
      const sr = el.shadowRoot;
      if (!sr) continue;
      const msg = sr.querySelector(".msg");
      if (msg && !msg.hidden && msg.textContent) break;
      const painted =
        sr.querySelector(".dve-print-page") ||
        sr.querySelector(".xg-root") ||
        (sr.querySelector(".dom") && sr.querySelector(".dom").childElementCount > 0);
      if (painted && i >= 3) break;
    }
  }

  async function openBytes(buf, name) {
    setError("");
    lastBytes = buf;
    lastName = name || lastName;
    try {
      await el.loadBuffer(buf.slice(0), lastName);
      await settlePaint();
    } catch (e) {
      setError(e?.message || String(e));
    }
  }

  async function openSample() {
    const url = new URL(SAMPLE[format], import.meta.url);
    const res = await fetch(url);
    if (!res.ok) throw new Error(`sample fetch failed: ${res.status}`);
    await openBytes(await res.arrayBuffer(), SAMPLE[format].split("/").pop());
  }

  qs("#pick")?.addEventListener("change", async (ev) => {
    const file = ev.target.files && ev.target.files[0];
    if (!file) return;
    try {
      await openBytes(await file.arrayBuffer(), file.name);
    } catch (e) {
      setError(e?.message || String(e));
    }
    ev.target.value = "";
  });

  qs("#reload-sample")?.addEventListener("click", () => {
    openSample().catch((e) => setError(e?.message || String(e)));
  });

  document.querySelectorAll("[data-mode-btn]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const mode = btn.dataset.modeBtn === "embedded" ? "embedded" : "full";
      document.querySelectorAll("[data-mode-btn]").forEach((b) => {
        b.setAttribute("aria-pressed", b.dataset.modeBtn === mode ? "true" : "false");
      });
      if (title) {
        title.textContent = `${format.toUpperCase()} · ${mode === "embedded" ? "embedded" : "normal"} mode`;
      }
      // Rebuild the host so chrome / embedded toolbar remount cleanly on the same bytes.
      const next = document.createElement(tag);
      next.setAttribute("readonly", "");
      next.setAttribute("mode", mode);
      next.style.width = "100%";
      next.style.height = "100%";
      stage.replaceChildren(next);
      el = next;
      if (lastBytes) {
        await openBytes(lastBytes, lastName);
      }
      const path =
        mode === "embedded"
          ? `./${format}-embedded.html`
          : `./${format}-full.html`;
      history.replaceState(null, "", path);
    });
  });

  try {
    await openSample();
  } catch (e) {
    setError(e?.message || String(e));
  }
}

export function shell(format, mode) {
  const label = format.toUpperCase();
  const modeLabel = mode === "embedded" ? "embedded" : "normal";
  document.title = `Nitro XML · ${label} ${modeLabel}`;
  document.body.innerHTML = `
<header class="top">
  <a href="./index.html">← Demos</a>
  <h1 id="title">${label} · ${modeLabel} mode</h1>
  <span class="grow"></span>
  <a href="https://github.com/jgridifier/nitro-xml">Repo</a>
  <a href="../index.html">Plan</a>
</header>
<div class="bar">
  <button type="button" data-mode-btn="full" aria-pressed="${mode === "full" ? "true" : "false"}">Normal</button>
  <button type="button" data-mode-btn="embedded" aria-pressed="${mode === "embedded" ? "true" : "false"}">Embedded</button>
  <button type="button" id="reload-sample">Sample file</button>
  <button type="button" id="open-local">Open local file</button>
  <input id="pick" type="file" accept=".${format},.${format === "xlsx" ? "xlsm,.xltx,.xltm" : format === "docx" ? "docm,.dotx,.dotm" : "pptm,.potx,.potm"}" hidden />
  <span class="hint">Files stay in your browser — nothing is uploaded.</span>
</div>
<div id="err" class="err" role="alert"></div>
<div id="stage" class="stage"></div>
`;
  qs("#open-local")?.addEventListener("click", () => qs("#pick")?.click());
  return boot({ format, mode });
}
