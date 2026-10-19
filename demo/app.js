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
  docx: "./samples/notes.docx",
  pptx: "./samples/deck.pptx",
};

const LOADER = "./nitro-xml/loader/";

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

export async function boot(opts) {
  const format = opts.format;
  let mode = opts.mode === "embedded" ? "embedded" : "full";
  const app = document.getElementById("app");
  const [loader, office] = await Promise.all([
    import(LOADER + "index.js"),
    import(LOADER + "lazy/ribbon/index.js"),
  ]);
  office.setTheme(office.getTheme());
  await loader.initCore({ fontSource: false });
  loader[DEFINE[format]]();

  const tag = TAG[format];
  const makeEl = () => {
    const e = document.createElement(tag);
    e.setAttribute("readonly", "");
    e.setAttribute("mode", mode);
    return e;
  };
  /** @type {ArrayBuffer | null} */
  let lastBytes = null;
  let lastName = SAMPLE[format].split("/").pop();
  let ui = null;
  let token = 0;

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

  async function settlePaint(el) {
    if (format === "xlsx") await injectXlsxGridCss(el);
    // Match visual harness: push Resize then tick so print/slide layout paints.
    const d = el.document;
    if (d) {
      const dpr = window.devicePixelRatio || 1;
      const w = el.clientWidth || 800;
      const h = el.clientHeight || 600;
      d.pushEvent({ kind: 6, time_ms: performance.now(), width: w, height: h, dpr });
    }
    for (let i = 0; i < 24; i++) {
      if (typeof el._tick === "function") el._tick();
      await waitFrames(1);
      const sr = el.shadowRoot;
      if (!sr) continue;
      const msg = sr.querySelector(".msg");
      if (msg && !msg.hidden && msg.textContent) return msg.textContent;
      const painted =
        sr.querySelector(".dve-print-page") ||
        sr.querySelector(".xg-root") ||
        (sr.querySelector(".dom") && sr.querySelector(".dom").childElementCount > 0);
      if (painted && i >= 3) break;
    }
    return "";
  }

  async function openBytes(buf, name) {
    const mine = ++token;
    lastBytes = buf;
    lastName = name || lastName;
    ui.loading(`Opening ${lastName}…`);
    try {
      const el = ui.el;
      await el.loadBuffer(buf.slice(0), lastName);
      const problem = await settlePaint(el);
      if (mine !== token) return;
      if (problem) throw new Error(problem);
      ui.ready();
      ui.doc(lastName, "Read-only");
    } catch (e) {
      if (mine === token) ui.error(`Couldn’t open ${lastName}: ${e?.message || e}`);
    }
  }

  async function openSample() {
    ui.loading("Opening the sample…");
    try {
      const url = new URL(SAMPLE[format], import.meta.url);
      const res = await fetch(url);
      if (!res.ok) throw new Error(`sample fetch failed: ${res.status}`);
      await openBytes(await res.arrayBuffer(), SAMPLE[format].split("/").pop());
    } catch (e) {
      ui.error(e?.message || String(e));
    }
  }

  function mount() {
    ui?.destroy();
    document.title = `Nitro XML · ${office.APP_INFO[format].name} · ${mode === "embedded" ? "simplified" : "full"} toolbar`;
    ui = office.mountOffice({
      container: app,
      el: makeEl(),
      format,
      mode,
      chrome: mode === "full",
      links: [
        ["./index.html", "All demos"],
        ["https://github.com/jgridifier/nitro-xml", "GitHub"],
      ],
      onMode(m) {
        if (m === mode) return;
        mode = m;
        history.replaceState(null, "", `./${format}-${m === "embedded" ? "embedded" : "full"}.html`);
        mount();
        if (lastBytes) openBytes(lastBytes, lastName);
        else ui.empty();
      },
      onFile: async (f) => openBytes(await f.arrayBuffer(), f.name),
      onSample: openSample,
      onClose() {
        token++;
        lastBytes = null;
        ui.doc("", "");
        ui.empty();
      },
    });
    ui.empty();
  }

  mount();
  await openSample();
}

export function shell(format, mode) {
  document.body.innerHTML = '<div id="app" class="demo-root"></div>';
  return boot({ format, mode });
}
