// SPDX-License-Identifier: MIT
// Declarative toolbar layout per format (PowerPoint Home order). Item fields:
//   i icon · l label · t tooltip · o shellAction op · a op arg · c ribbon command id (xlsx cmd)
//   p toggle (aria-pressed) · x op sent instead of `o` while pressed · r readout key · u no ribbon control to mirror
// xlsx toggles carry no `p`: the Rust chrome has no live cell-style state yet (is_checked is a stub), so a pressed look would lie.
// Items with `c` are shown only while the Rust ribbon has that command (editable session);
// mirrored ones also follow that control's enabled state. Do not list an id the editor does not emit.
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

const file = {
  id: "File",
  items: [
    cmd("file.save", "save", "Save", "Save (Ctrl+S)"),
    cmd("edit.undo", "undo", "Undo", "Undo (Ctrl+Z)"),
    cmd("edit.redo", "redo", "Redo", "Redo (Ctrl+Y)"),
  ],
};
const font = {
  id: "Font",
  items: [
    cmd("home.bold", "bold", "Bold", "Bold (Ctrl+B)"),
    cmd("home.italic", "italic", "Italic", "Italic (Ctrl+I)"),
    cmd("home.underline", "underline", "Underline", "Underline (Ctrl+U)"),
    cmd("home.strikethrough", "strike", "Strikethrough", "Strikethrough (Ctrl+5)"),
    cmd("home.font-size", "grow", "Font size", "Set the font size"),
    cmd("home.font-color", "color", "Font color", "Set the font color"),
    cmd("home.indent", "indent", "Increase indent"),
    cmd("home.outdent", "outdent", "Decrease indent"),
    cmd("home.line-spacing", "spacing", "Line spacing", "Cycle line spacing"),
    cmd("home.subscript", "subscript", "Subscript"),
    cmd("home.superscript", "superscript", "Superscript"),
    cmd("home.font-name", "font", "Font", "Set the font name"),
  ],
};
const align = {
  id: "Alignment",
  items: [
    cmd("home.align-left", "left", "Align left"),
    cmd("home.align-center", "center", "Center"),
    cmd("home.align-right", "right", "Align right"),
    cmd("home.align-justify", "justify", "Justify"),
  ],
};

export const MANIFEST = {
  docx: {
    label: "Document tools",
    groups: [
      file,
      copy,
      {
        id: "Font",
        items: [...font.items, cmd("home.highlight", "highlight", "Highlight", "Text highlight")],
      },
      align,
      {
        id: "Lists",
        items: [cmd("home.bullets", "bullet", "Bullets"), cmd("home.numbering", "numbering", "Numbering")],
      },
      {
        id: "Tables",
        items: [
          cmd("insert.picture", "picture", "Picture", "Insert a picture"),
          cmd("insert.table", "table", "Table", "Insert a table"),
          cmd("table.insert-row", "rowAdd", "Insert row"),
          cmd("table.delete-row", "rowDel", "Delete row"),
          cmd("table.insert-column", "colAdd", "Insert column"),
          cmd("table.delete-column", "colDel", "Delete column"),
          cmd("table.delete", "tableDel", "Delete table"),
        ],
      },
      nav("Pages", "page"),
      { id: "Editing", items: [find] },
      view,
    ],
  },
  pptx: {
    label: "Presentation tools",
    groups: [
      file,
      copy,
      font,
      align,
      {
        id: "Lists",
        items: [
          cmd("home.bullets", "bullet", "Bullets"),
          cmd("home.numbering", "numbering", "Numbering"),
        ],
      },
      {
        id: "Slides",
        items: [
          cmd("insert.slide", "slide", "New slide"),
          cmd("insert.duplicate-slide", "duplicate", "Duplicate slide"),
          cmd("insert.move-slide-up", "chevUp", "Move slide up"),
          cmd("insert.move-slide-down", "chevDown", "Move slide down"),
          cmd("insert.text-box", "text", "Text box"),
          cmd("insert.picture", "picture", "Picture", "Insert a picture"),
          cmd("home.delete-slide", "trash", "Delete slide"),
        ],
      },
      {
        id: "Tables",
        items: [
          cmd("insert.table", "table", "Table", "Insert a table"),
          cmd("table.insert-row", "rowAdd", "Insert row"),
          cmd("table.delete-row", "rowDel", "Delete row"),
          cmd("table.insert-column", "colAdd", "Insert column"),
          cmd("table.delete-column", "colDel", "Delete column"),
        ],
      },
      nav("Slides", "slide"),
      { id: "Editing", items: [find] },
      view,
    ],
  },
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
