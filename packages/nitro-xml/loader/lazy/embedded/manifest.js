// SPDX-License-Identifier: MIT
// Declarative toolbar layout per format (PowerPoint Home order). Item fields:
//   i icon · l label · t tooltip · o shellAction op · a op arg · c ribbon command id (xlsx cmd)
//   p toggle (aria-pressed) · x op sent instead of `o` while pressed · r readout key · u no ribbon control to mirror
//   v view-only shell control, hidden once the editor emits home.copy
// xlsx toggles carry no `p`: the Rust chrome has no live cell-style state yet (is_checked is a stub), so a pressed look would lie.
// Items with `c` are shown only while the Rust ribbon has that command (editable session);
// mirrored ones also follow that control's enabled state. Do not list an id the editor does not emit.
const cmd = (c, i, l, t, p, u) => ({ c, i, l, t: t || l, p, u });
const copy = { id: "Clipboard", items: [{ i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy" }] };
// `v` is the view-only shell Copy. Cut/Copy/Paste commands hide until the editor emits them,
// so a read-only document still has one Copy for the keyboard path (Find / Next stay reachable).
const viewCopy = { i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy", v: 1 };
const editorClip = {
  id: "Clipboard",
  items: [
    cmd("home.cut", "cut", "Cut", "Cut (Ctrl+X)"),
    cmd("home.copy", "copy", "Copy", "Copy (Ctrl+C)"),
    viewCopy,
    cmd("home.paste", "paste", "Paste", "Paste (Ctrl+V)"),
  ],
};
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
    cmd("home.grow-font", "grow", "Grow font", "Increase the font size"),
    cmd("home.shrink-font", "shrink", "Shrink font", "Decrease the font size"),
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
      editorClip,
      {
        id: "Font",
        items: [
          ...font.items,
          cmd("home.highlight", "highlight", "Highlight", "Text highlight"),
          cmd("home.shading", "shade", "Shade", "Shade the paragraph yellow"),
          cmd("home.border-bottom", "borderBottom", "Bottom border", "Toggle the paragraph bottom border"),
          cmd("home.border-top", "borderTop", "Top border", "Toggle the paragraph top border"),
          cmd("home.border-left", "borderLeft", "Left border", "Toggle the paragraph left border"),
          cmd("home.border-right", "borderRight", "Right border", "Toggle the paragraph right border"),
          cmd("home.border-double", "borderDouble", "Double border", "Toggle a double paragraph bottom border"),
          cmd("home.border-dash", "borderDash", "Dashed border", "Toggle a dashed paragraph bottom border"),
          cmd("home.page-break-before", "pageBreakBefore", "Page break before", "Start this paragraph on a new page"),
          cmd("home.space-before", "spaceBefore", "Space before", "Add 12 pt space before the paragraph"),
          cmd("home.space-after", "spaceAfter", "Space after", "Add 12 pt space after the paragraph"),
          cmd("home.hanging-indent", "hanging", "Hanging", "Hanging indent, half an inch"),
          cmd("home.first-line", "firstLine", "First line", "First line indent, half an inch"),
          cmd("home.indent-right", "indentRight", "Increase right", "Increase the right indent by half an inch"),
          cmd("home.outdent-right", "outdentRight", "Decrease right", "Decrease the right indent by half an inch"),
          cmd("home.double-underline", "doubleUnderline", "Double underline", "Double underline"),
          cmd("home.caps", "caps", "All caps", "All caps"),
          cmd("home.hidden", "hidden", "Hidden", "Hidden text"),
          cmd("home.expand-spacing", "expandSpacing", "Expand", "Expand character spacing"),
          cmd("home.condense-spacing", "condenseSpacing", "Condense", "Condense character spacing"),
          cmd("home.scale-up", "scaleUp", "Scale up", "Increase character scale"),
          cmd("home.scale-down", "scaleDown", "Scale down", "Decrease character scale"),
        ],
      },
      align,
      {
        id: "Styles",
        items: [
          cmd("home.style-normal", "style", "Normal"),
          cmd("home.style-title", "style", "Title"),
          cmd("home.style-heading-1", "style", "Heading 1"),
          cmd("home.style-heading-2", "style", "Heading 2"),
          cmd("home.style-heading-3", "style", "Heading 3"),
        ],
      },
      {
        id: "Lists",
        items: [cmd("home.bullets", "bullet", "Bullets"), cmd("home.numbering", "numbering", "Numbering")],
      },
      {
        id: "Tables",
        items: [
          cmd("insert.page-break", "pageBreak", "Page break", "Insert a page break"),
          cmd("insert.picture", "picture", "Picture", "Insert a picture"),
          cmd("insert.table", "table", "Table", "Insert a table"),
          cmd("table.insert-row", "rowAdd", "Insert row"),
          cmd("table.delete-row", "rowDel", "Delete row"),
          cmd("table.insert-column", "colAdd", "Insert column"),
          cmd("table.delete-column", "colDel", "Delete column"),
          cmd("table.cell-text", "cellText", "Cell text", "Set cell text"),
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
      editorClip,
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
        id: "Drawing",
        items: [
          cmd("home.shape-fill", "fill", "Fill", "Shape fill"),
          cmd("home.shape-outline", "outline", "Outline", "Shape outline"),
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
          cmd("insert.shape", "shape", "Shape", "Insert a shape"),
          cmd("insert.picture", "picture", "Picture", "Insert a picture"),
          cmd("home.layout", "layout", "Layout", "Change slide layout"),
          cmd("home.delete-slide", "trash", "Delete slide"),
        ],
      },
      {
        id: "Arrange",
        items: [
          cmd("home.duplicate", "duplicate", "Duplicate", "Duplicate shape"),
          cmd("home.delete", "trash", "Delete", "Delete shape"),
          cmd("home.bring-forward", "forward", "Bring forward"),
          cmd("home.send-backward", "back", "Send backward"),
          cmd("home.bring-to-front", "front", "Bring to front"),
          cmd("home.send-to-back", "toBack", "Send to back"),
          cmd("home.group", "group", "Group", "Group the selected shapes"),
          cmd("home.ungroup", "ungroup", "Ungroup", "Ungroup the selected shapes"),
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
          cmd("table.cell-text", "cellText", "Cell text", "Set cell text"),
          cmd("table.delete", "tableDel", "Delete table", "Delete the table"),
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
