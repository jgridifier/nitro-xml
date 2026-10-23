// SPDX-License-Identifier: MIT
// Ribbon manifests for the Office-style frame. A control is listed only if it maps to a real
// action: a viewer `shellAction` op (o) or an editor ribbon command id (c, sent as shellAction("cmd", id)).
// Every `c` id below was checked against the engine (the saved bytes change, or copy reads them).
// All caps is a Word command that writes w:caps.
// Delete Table removes the slide table and leaves other shapes.
// Cut, paste, and group rewrite the package; copy only fills the editor clipboard.
// Grow and shrink font, character spacing, character scale, indent, right indent, line spacing,
// paragraph space before and after, paragraph shading, paragraph bottom, top, and left borders, double bottom border, page break before,
// hanging indent, first-line indent, double underline, all caps, hidden text, script, font name,
// Word bullets, insert shape, duplicate,
// delete, z-order (one step and all the way), shape fill, outline, and slide layout are listed
// only after they change the saved package.
//   i icon · l label · t tooltip · o op · a op arg · x op sent while pressed · p toggle · n tab step
//   c editor cmd id · d diagnostics toggle · s slide-pane toggle (host) · m dropdown entries · r readout
//   g group caption · big: large buttons · rows: stacked small buttons · sm: small button style
const it = (c, i, l, t, extra) => ({ c, i, l, t: t || l, ...extra });

const nav = (unit) => ({
  g: unit + "s",
  big: [
    { i: "prev", l: "Previous", t: `Previous ${unit.toLowerCase()}`, o: "nav-prev", n: -1 },
    { r: "nav" },
    { i: "next", l: "Next", t: `Next ${unit.toLowerCase()}`, o: "nav-next", n: 1 },
  ],
});
const find = { g: "Editing", big: [{ i: "find", l: "Find", t: "Find (Ctrl+F)", o: "find-open", x: "find-close", p: 1 }] };
const lists = {
  g: "Paragraph",
  rows: [[it("home.bullets", "bullet", "Bullets", "Bulleted list"), it("home.numbering", "numbering", "Numbering", "Numbered list")]],
};
const drawing = {
  g: "Drawing",
  rows: [[
    it("home.shape-fill", "fill", "Fill", "Shape fill"),
    it("home.shape-outline", "outline", "Outline", "Shape outline"),
  ]],
};
const arrange = {
  g: "Arrange",
  big: [
    it("home.duplicate", "duplicate", "Duplicate", "Duplicate shape"),
    it("home.delete", "trash", "Delete", "Delete shape"),
    it("home.bring-forward", "forward", "Bring Forward"),
    it("home.send-backward", "back", "Send Backward"),
    it("home.bring-to-front", "front", "Bring to Front"),
    it("home.send-to-back", "toBack", "Send to Back"),
    it("home.group", "group", "Group", "Group the selected shapes"),
    it("home.ungroup", "ungroup", "Ungroup", "Ungroup the selected shapes"),
  ],
};
const wordLists = {
  g: "Paragraph",
  rows: [[it("home.bullets", "bullet", "Bullets", "Bulleted list"), it("home.numbering", "numbering", "Numbering", "Numbered list")]],
};
const metrics = [
  it("home.indent", "indent", "Increase indent"),
  it("home.outdent", "outdent", "Decrease indent"),
  it("home.line-spacing", "spacing", "Line spacing", "Cycle line spacing"),
  it("home.subscript", "subscript", "Subscript"),
  it("home.superscript", "superscript", "Superscript"),
  it("home.font-name", "font", "Font", "Set the font name"),
];
const wordStyles = {
  g: "Styles",
  rows: [[
    it("home.style-normal", "style", "Normal"),
    it("home.style-title", "style", "Title"),
    it("home.style-heading-1", "style", "Heading 1"),
    it("home.style-heading-2", "style", "Heading 2"),
    it("home.style-heading-3", "style", "Heading 3"),
  ]],
};
const copy = { i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy" };
const editorClip = {
  g: "Clipboard",
  big: [
    it("home.cut", "cut", "Cut", "Cut (Ctrl+X)"),
    it("home.copy", "copy", "Copy", "Copy (Ctrl+C)"),
    it("home.paste", "paste", "Paste", "Paste (Ctrl+V)"),
  ],
};

const FONT = {
  g: "Font",
  rows: [
    [
      it("home.bold", "bold", "Bold", "Bold (Ctrl+B)"),
      it("home.italic", "italic", "Italic", "Italic (Ctrl+I)"),
      it("home.underline", "underline", "Underline", "Underline (Ctrl+U)"),
      it("home.strikethrough", "strike", "Strikethrough", "Strikethrough (Ctrl+5)"),
    ],
    [
      it("home.align-left", "left", "Align left"),
      it("home.align-center", "center", "Center"),
      it("home.align-right", "right", "Align right"),
      it("home.align-justify", "justify", "Justify"),
      it("home.font-size", "grow", "Font size", "Set the font size"),
      it("home.grow-font", "grow", "Grow font", "Increase the font size"),
      it("home.shrink-font", "shrink", "Shrink font", "Decrease the font size"),
      it("home.font-color", "color", "Font color", "Set the font color"),
    ],
    metrics,
  ],
};
const wordFont = {
  g: "Font",
  rows: [
    [
      ...FONT.rows[0],
      it("home.double-underline", "doubleUnderline", "Double underline", "Double underline"),
      it("home.caps", "caps", "All caps", "All caps"),
      it("home.hidden", "hidden", "Hidden", "Hidden text"),
      it("home.expand-spacing", "expandSpacing", "Expand", "Expand character spacing"),
      it("home.condense-spacing", "condenseSpacing", "Condense", "Condense character spacing"),
      it("home.scale-up", "scaleUp", "Scale up", "Increase character scale"),
      it("home.scale-down", "scaleDown", "Scale down", "Decrease character scale"),
      it("home.highlight", "highlight", "Highlight", "Text highlight"),
    ],
    FONT.rows[1],
    FONT.rows[2],
    [
      it("home.shading", "shade", "Shade", "Shade the paragraph yellow"),
      it("home.border-bottom", "borderBottom", "Bottom border", "Toggle the paragraph bottom border"),
      it("home.border-top", "borderTop", "Top border", "Toggle the paragraph top border"),
      it("home.border-left", "borderLeft", "Left border", "Toggle the paragraph left border"),
      it("home.border-double", "borderDouble", "Double border", "Toggle a double paragraph bottom border"),
      it("home.page-break-before", "pageBreakBefore", "Page break before", "Start this paragraph on a new page"),
      it("home.space-before", "spaceBefore", "Space before", "Add 12 pt space before the paragraph"),
      it("home.space-after", "spaceAfter", "Space after", "Add 12 pt space after the paragraph"),
      it("home.hanging-indent", "hanging", "Hanging", "Hanging indent, half an inch"),
      it("home.first-line", "firstLine", "First line", "First line indent, half an inch"),
      it("home.indent-right", "indentRight", "Increase right", "Increase the right indent by half an inch"),
      it("home.outdent-right", "outdentRight", "Decrease right", "Decrease the right indent by half an inch"),
    ],
  ],
};

const view = (pptx) => [
  {
    g: "Zoom",
    big: [
      { i: "zoomOut", l: "Zoom out", t: "Zoom out (Ctrl+-)", o: "zoom-out" },
      { r: "zoom" },
      { i: "zoomIn", l: "Zoom in", t: "Zoom in (Ctrl++)", o: "zoom-in" },
      { i: "zoom100", l: "100%", t: "Zoom to 100%", o: "zoom-set", a: "100" },
    ],
  },
  {
    g: "Show",
    big: [
      ...(pptx ? [{ i: "slides", l: "Slide pane", t: "Show or hide the slide pane", s: 1, p: 1 }] : []),
      { i: "diag", l: "Diagnostics", t: "Show or hide load diagnostics", o: "diagnostics-toggle", p: 1, d: 1 },
    ],
  },
];

/** Number formats the engine really applies (`home.format-*`). */
export const NUMBER_FORMATS = [
  it("home.format-general", "numFmt", "General"),
  it("home.format-number", "numFmt", "Number", "Number (0.00)"),
  it("home.format-currency", "numFmt", "Currency", "Currency ($)"),
  it("home.accounting", "numFmt", "Accounting"),
  it("home.percent", "numFmt", "Percentage", "Percentage (%)"),
  it("home.comma", "numFmt", "Comma", "Comma style"),
  it("home.format-scientific", "numFmt", "Scientific", "Scientific (0.00E+00)"),
  it("home.format-date", "numFmt", "Date", "Short date"),
  it("home.format-time", "numFmt", "Time"),
];

/** Quick-access bar (title bar), editable workbooks only. */
export const QAT = [
  it("file.save", "save", "Save", "Save (Ctrl+S): download the edited file"),
  it("edit.undo", "undo", "Undo", "Undo (Ctrl+Z)"),
  it("edit.redo", "redo", "Redo", "Redo (Ctrl+Y)"),
];

const XLSX_HOME = [
  { g: "Clipboard", big: [copy] },
  {
    g: "Font",
    rows: [
      [it("home.bold", "bold", "Bold", "Bold (Ctrl+B)"), it("home.italic", "italic", "Italic", "Italic (Ctrl+I)"), it("home.underline", "underline", "Underline", "Underline (Ctrl+U)"), it("home.strikethrough", "strike", "Strikethrough", "Strikethrough (Ctrl+5)")],
      [it("home.grow-font", "grow", "Increase font size"), it("home.shrink-font", "shrink", "Decrease font size")],
    ],
  },
  {
    g: "Alignment",
    rows: [
      [it("home.align-top", "top", "Top align"), it("home.align-middle", "middle", "Middle align"), it("home.align-bottom", "bottom", "Bottom align"), it("home.wrap-text", "wrap", "Wrap text")],
      [it("home.align-left", "left", "Align left"), it("home.align-center", "center", "Center"), it("home.align-right", "right", "Align right")],
    ],
  },
  {
    g: "Number",
    rows: [
      [{ i: "numFmt", l: "Number format", t: "Number format", m: NUMBER_FORMATS, wide: 1, sm: 1 }],
      [it("home.accounting", "accounting", "Accounting number format"), it("home.percent", "percent", "Percent style"), it("home.comma", "comma", "Comma style")],
    ],
  },
  { g: "Sheets", big: [it("sheet.insert", "sheetAdd", "Insert sheet"), it("sheet.delete", "sheetDel", "Delete sheet")] },
  { g: "Editing", big: [it("home.clear-contents", "clear", "Clear", "Clear contents"), find.big[0]] },
];

/**
 * @param {'xlsx'|'docx'|'pptx'} format
 * @param {string} unit  Sheet | Page | Slide
 * @param {boolean} edit editor commands are wired for this format and the element is writable
 */
/**
 * Hide a Pages command once the WASM strip is up and does not emit it.
 * PowerPoint and Word only. Excel keeps its own controls. Before chrome
 * HTML arrives the button stays (the caller disables it) so the ribbon
 * does not flash empty on open.
 */
export function officeCommandHidden(format, chromeReady, present) {
  if (format !== "pptx" && format !== "docx") return false;
  if (!chromeReady) return false;
  return !present;
}

export function tabsFor(format, unit, edit) {
  const viewOnly = { Home: [{ g: "Clipboard", big: [copy] }, nav(unit), find] };
  const officeHome = [editorClip, FONT, nav(unit), find];
  const tabs =
    format === "xlsx" && edit
      ? { Home: [...XLSX_HOME.slice(0, 5), nav(unit), XLSX_HOME[5]] }
      : format === "pptx" && edit
        ? {
            Home: [...officeHome.slice(0, 2), lists, drawing, arrange, ...officeHome.slice(2)],
            Insert: [
              {
                g: "Slides",
                big: [
                  it("insert.slide", "slide", "New Slide"),
                  it("insert.duplicate-slide", "duplicate", "Duplicate Slide"),
                  it("insert.move-slide-up", "chevUp", "Move Slide Up"),
                  it("insert.move-slide-down", "chevDown", "Move Slide Down"),
                  it("insert.text-box", "text", "Text Box"),
                  it("insert.shape", "shape", "Shape", "Insert a shape"),
                  it("insert.picture", "picture", "Picture", "Insert a picture"),
                  it("home.layout", "layout", "Layout", "Change slide layout"),
                  it("home.delete-slide", "trash", "Delete Slide"),
                ],
              },
              {
                g: "Tables",
                big: [
                  it("insert.table", "table", "Table", "Insert a table"),
                  it("table.insert-row", "rowAdd", "Insert Row"),
                  it("table.delete-row", "rowDel", "Delete Row"),
                  it("table.insert-column", "colAdd", "Insert Column"),
                  it("table.delete-column", "colDel", "Delete Column"),
                  it("table.cell-text", "cellText", "Cell Text", "Set cell text"),
                  it("table.delete", "tableDel", "Delete Table", "Delete the table"),
                ],
              },
            ],
          }
        : format === "docx" && edit
          ? {
              Home: [officeHome[0], wordFont, wordStyles, wordLists, ...officeHome.slice(2)],
              Insert: [
                {
                  g: "Pages",
                  big: [it("insert.page-break", "pageBreak", "Page Break", "Insert a page break")],
                },
                {
                  g: "Tables",
                  big: [
                    it("insert.picture", "picture", "Picture", "Insert a picture"),
                    it("insert.table", "table", "Table", "Insert a table"),
                    it("table.insert-row", "rowAdd", "Insert Row"),
                    it("table.delete-row", "rowDel", "Delete Row"),
                    it("table.insert-column", "colAdd", "Insert Column"),
                    it("table.delete-column", "colDel", "Delete Column"),
                    it("table.cell-text", "cellText", "Cell Text", "Set cell text"),
                    it("table.delete", "tableDel", "Delete Table"),
                  ],
                },
              ],
            }
          : viewOnly;
  tabs.View = view(format === "pptx");
  // Give every control a stable key; groups normalise to { big, rows } lists.
  let n = 0;
  for (const [tab, groups] of Object.entries(tabs)) {
    for (const g of groups) {
      g.big = g.big || [];
      g.rows = g.rows || [];
      for (const x of [...g.big, ...g.rows.flat()]) {
        if (x.r) continue;
        x.k = `${tab}:${n++}`;
        for (const m of x.m || []) m.k = `${tab}:${n++}`;
      }
    }
  }
  return tabs;
}
