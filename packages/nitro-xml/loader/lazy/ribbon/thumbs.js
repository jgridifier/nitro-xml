// SPDX-License-Identifier: MIT
// Real slide thumbnails for the pptx pane, with no renderer change. A second, hidden <nitro-pptx>
// opens the same bytes and steps through the slides with the existing `sheet` shell action; after
// each paint the slide's DOM (the renderer lays a slide out as a 1280x720 box scaled by a CSS
// transform) is cloned into a small shadow root and re-scaled. One slide at a time, only for
// thumbnails that scroll into view, so opening a large deck costs nothing until the pane is read.
const tick = (n) => new Promise((r) => (n > 0 ? requestAnimationFrame(() => tick(n - 1).then(r)) : r()));

export function createThumbs(TAG = "nitro-pptx") {
  let host = null;
  let ready = null;
  let queue = Promise.resolve();
  let gen = 0;
  const cache = new Map();
  const done = new Set(); // slides whose thumbnail rendered

  function start(bytes, name) {
    stop();
    const mine = gen;
    host = document.createElement(TAG);
    host.setAttribute("readonly", "");
    host.setAttribute("aria-hidden", "true");
    host.inert = true;
    // Off-screen but laid out: the renderer sizes slides from the host's real box.
    host.style.cssText = "position:fixed;left:-4000px;top:0;width:640px;height:400px;visibility:hidden;pointer-events:none";
    document.body.append(host);
    ready = host.loadBuffer(bytes.slice(0), name).then(() => mine === gen);
  }
  function stop() {
    gen++;
    cache.clear();
    done.clear();
    ready = null;
    queue = Promise.resolve();
    if (host) {
      host.remove();
      host = null;
    }
  }

  async function paint(g) {
    const el = host;
    await el.shellAction("sheet", g);
    for (let i = 0; i < 20; i++) {
      if (el.document) el.document.pushEvent({ kind: 6, time_ms: performance.now(), width: 640, height: 400, dpr: 1 });
      if (typeof el._tick === "function") el._tick();
      await tick(2);
      const slide = el.shadowRoot && el.shadowRoot.querySelector(".dom > [aria-roledescription=slide]");
      if (slide && i >= 1) return slide;
    }
    return null;
  }

  /** Resolves a ready-to-insert <span> (own shadow root) showing slide `g`, or null. */
  function render(g) {
    if (cache.has(g)) return cache.get(g);
    const job = queue.then(async () => {
      const mine = gen;
      if (!ready || !(await ready) || mine !== gen) return null;
      const slide = await paint(g);
      if (!slide || mine !== gen) return null;
      done.add(String(g));
      const w = parseFloat(slide.style.width) || 1280;
      const h = parseFloat(slide.style.height) || 720;
      const clone = slide.cloneNode(true);
      // cloneNode drops canvas pixels (charts, effects): copy them across.
      const from = slide.querySelectorAll("canvas");
      clone.querySelectorAll("canvas").forEach((c, i) => from[i] && c.getContext("2d").drawImage(from[i], 0, 0));
      clone.removeAttribute("aria-label");
      clone.setAttribute("aria-hidden", "true");
      for (const n of clone.querySelectorAll("[id]")) n.removeAttribute("id");
      clone.style.position = "absolute";
      clone.style.left = clone.style.top = "0";
      clone.style.transformOrigin = "0 0";
      clone.style.pointerEvents = "none";
      const css = [...host.shadowRoot.querySelectorAll("style")].filter((s) => s.textContent.includes("pptxv")).map((s) => s.cloneNode(true));
      return { clone, css, w, h };
    });
    // Keep the queue alive if a job fails.
    queue = job.catch(() => null);
    cache.set(g, job);
    return job;
  }

  /** Mount a rendered thumbnail into `box`, scaled to the box width. */
  function mount(box, r) {
    if (!r) return;
    const root = box.shadowRoot || box.attachShadow({ mode: "open" });
    root.replaceChildren(...r.css.map((s) => s.cloneNode(true)), r.clone.cloneNode(true));
    box.dataset.live = "1";
    box._thumb = r;
    fit(box);
  }
  function fit(box) {
    const r = box._thumb;
    if (!r || !box.shadowRoot) return;
    const k = box.clientWidth / r.w;
    if (!k) return;
    const s = box.shadowRoot.lastElementChild;
    if (s) s.style.transform = `scale(${k})`;
  }

  /**
   * A slide moved: reopen the hidden host on the reordered bytes and carry over the thumbnails
   * already rendered, under their new positions.
   */
  function reorder(from, to, bytes, name) {
    const at = (g) => (g === from ? to : from < to ? (g > from && g <= to ? g - 1 : g) : g >= to && g < from ? g + 1 : g);
    const kept = [...cache].filter(([g]) => done.has(String(g))).map(([g, job]) => [String(at(Number(g))), job]);
    start(bytes, name);
    for (const [g, job] of kept) {
      cache.set(g, job);
      done.add(g);
    }
  }

  return { start, stop, render, mount, fit, reorder };
}
