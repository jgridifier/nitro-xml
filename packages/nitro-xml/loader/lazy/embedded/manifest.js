// SPDX-License-Identifier: MIT
// Declarative toolbar layout per format (PowerPoint Home order). Item fields:
//   i icon · l label · t tooltip · o shellAction op · a op arg · c ribbon command id (xlsx cmd)
//   p toggle (aria-pressed) · x op sent instead of `o` while pressed · r readout key · u no ribbon control to mirror
// xlsx toggles carry no `p`: the Rust chrome has no live cell-style state yet (is_checked is a stub), so a pressed look would lie.
// Items with `c` are shown only while the Rust ribbon exists (editable xlsx); mirrored ones also follow that control's state.
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

const cmd = (c, i, l, t, p, u) => ({ c, i, l, t: t || l, p, u });

export const MANIFEST = {
  docx: { label: "Document tools", groups: [copy, nav("Pages", "page"), { id: "Editing", items: [find] }, view] },
  pptx: { label: "Presentation tools", groups: [copy, nav("Slides", "slide"), { id: "Editing", items: [find] }, view] },
  xlsx: {
    label: "Workbook tools",
    groups: [
      { id: "File", items: [cmd("file.save", "save", "Save", "Save (Ctrl+S)", 0, 1), cmd("edit.undo", "undo", "Undo", "Undo (Ctrl+Z)", 0, 1), cmd("edit.redo", "redo", "Redo", "Redo (Ctrl+Y)", 0, 1)] },
      copy,
      {
        id: "Font",
        items: [
          cmd("home.bold", "bold", "Bold", "Bold (Ctrl+B)"),
          cmd("home.italic", "italic", "Italic", "Italic (Ctrl+I)"),
          cmd("home.underline", "underline", "Underline", "Underline (Ctrl+U)"),
          cmd("home.strikethrough", "strike", "Strike", "Strikethrough (Ctrl+5)"),
          cmd("home.grow-font", "grow", "Larger", "Larger font"),
          cmd("home.shrink-font", "shrink", "Smaller", "Smaller font"),
        ],
      },
      {
        id: "Alignment",
        items: [
          cmd("home.align-top", "top", "Top", "Top align"),
          cmd("home.align-middle", "middle", "Middle", "Middle align"),
          cmd("home.align-bottom", "bottom", "Bottom", "Bottom align"),
          cmd("home.align-left", "left", "Left", "Align left"),
          cmd("home.align-center", "center", "Center", "Center"),
          cmd("home.align-right", "right", "Right", "Align right"),
          cmd("home.wrap-text", "wrap", "Wrap", "Wrap text"),
        ],
      },
      {
        id: "Number",
        items: [
          cmd("home.percent", "percent", "Percent", "Percent style"),
          cmd("home.comma", "comma", "Comma", "Comma style"),
          cmd("home.accounting", "accounting", "Accounting", "Accounting number format"),
        ],
      },
      { id: "Cells", items: [cmd("sheet.insert", "sheetAdd", "Insert sheet"), cmd("sheet.delete", "sheetDel", "Delete sheet")] },
      { id: "Editing", items: [cmd("home.clear-contents", "clear", "Clear", "Clear contents"), find] },
      view,
    ],
  },
};
