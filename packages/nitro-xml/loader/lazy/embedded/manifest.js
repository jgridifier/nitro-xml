// SPDX-License-Identifier: MIT
// Declarative toolbar layout per format (PowerPoint Home order). Item fields:
//   i icon · l label · t tooltip · o shellAction op · a op arg · c ribbon command id (xlsx cmd)
//   p toggle (aria-pressed) · x op sent instead of `o` while pressed · r readout key
// Items with `c` are shown only while the Rust ribbon exposes that control (editable xlsx).
const copy = { id: "Clipboard", items: [{ i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy" }] };
const find = { i: "find", l: "Find", t: "Find (Ctrl+F)", o: "find-open", x: "find-close", p: 1 };
const view = {
  id: "View",
  items: [
    { i: "zoomOut", l: "Zoom out", t: "Zoom out (Ctrl+-)", o: "zoom-out" },
    { r: "zoom" },
    { i: "zoomIn", l: "Zoom in", t: "Zoom in (Ctrl++)", o: "zoom-in" },
  ],
};
const nav = (id, noun) => ({
  id,
  items: [
    { i: "prev", l: "Previous", t: `Previous ${noun}`, o: "nav-prev", n: -1 },
    { r: "nav" },
    { i: "next", l: "Next", t: `Next ${noun}`, o: "nav-next", n: 1 },
  ],
});

export const MANIFEST = {
  docx: { label: "Document tools", groups: [copy, nav("Pages", "page"), { id: "Editing", items: [find] }, view] },
  pptx: { label: "Presentation tools", groups: [copy, nav("Slides", "slide"), { id: "Editing", items: [find] }, view] },
};
