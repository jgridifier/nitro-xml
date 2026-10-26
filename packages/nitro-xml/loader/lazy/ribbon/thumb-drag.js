// SPDX-License-Identifier: MIT
// Drag-to-reorder for the pptx slide pane. Pointer-driven (not HTML5 drag and drop) so it behaves
// the same in Chromium and WebKit; touch keeps scrolling the pane. Alt+Up / Alt+Down moves the
// focused slide one step for keyboard users. `move(from, to)` does the actual edit.
const SLOP = 5;

export function attachThumbDrag(pl, { enabled, move }) {
  let d = null;
  const items = () => [...pl.children];
  const clear = () => {
    for (const b of pl.children) {
      delete b.dataset.ins;
      delete b.dataset.drag;
    }
  };
  const slotAt = (y) => {
    const l = items();
    for (let i = 0; i < l.length; i++) {
      const r = l[i].getBoundingClientRect();
      if (y < r.top + r.height / 2) return i;
    }
    return l.length;
  };
  const mark = (slot) => {
    const l = items();
    clear();
    d.b.dataset.drag = "1";
    if (slot < l.length) l[slot].dataset.ins = "before";
    else if (l.length) l[l.length - 1].dataset.ins = "after";
  };
  pl.addEventListener("pointerdown", (e) => {
    const b = e.target.closest && e.target.closest("button[data-g]");
    if (e.button !== 0 || e.pointerType === "touch" || !b || !enabled()) return;
    d = { b, from: items().indexOf(b), x: e.clientX, y: e.clientY, on: false, slot: -1, id: e.pointerId };
  });
  pl.addEventListener("pointermove", (e) => {
    if (!d) return;
    if (!(e.buttons & 1)) {
      d = null;
      return clear();
    }
    if (!d.on) {
      if (Math.hypot(e.clientX - d.x, e.clientY - d.y) < SLOP) return;
      d.on = true;
      try {
        pl.setPointerCapture(d.id);
      } catch {
        /* capture is optional */
      }
    }
    d.slot = slotAt(e.clientY);
    mark(d.slot);
  });
  const end = (e) => {
    const g = d;
    d = null;
    clear();
    if (!g || !g.on) return;
    // The click that follows a drag must not navigate.
    const eat = (c) => {
      c.stopPropagation();
      c.preventDefault();
    };
    pl.addEventListener("click", eat, { capture: true, once: true });
    setTimeout(() => pl.removeEventListener("click", eat, true), 0);
    if (e.type === "pointercancel" || g.slot < 0) return;
    const to = g.slot > g.from ? g.slot - 1 : g.slot;
    if (to !== g.from) move(g.from, to);
  };
  pl.addEventListener("pointerup", end);
  pl.addEventListener("pointercancel", end);
  pl.addEventListener("keydown", (e) => {
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown") || !enabled()) return;
    const b = e.target.closest && e.target.closest("button[data-g]");
    if (!b) return;
    const from = items().indexOf(b);
    const to = from + (e.key === "ArrowUp" ? -1 : 1);
    if (to < 0 || to >= pl.children.length) return;
    e.preventDefault();
    move(from, to);
  });
}
