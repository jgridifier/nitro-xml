// SPDX-License-Identifier: MIT
// Ribbon manifests for the Office-style frame. A control is listed only if it maps to a real
// action: a viewer `shellAction` op (o) or an editor ribbon command id (c, sent as shellAction("cmd", id)).
// Every `c` id below was checked against the engine (the saved bytes change); anything the editor
// registers but does not implement (cut, paste, borders, shapes, grow/shrink, group, bring to front) is deliberately absent.
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
      it("home.font-color", "color", "Font color", "Set the font color"),
    ],
    metrics,
  ],
};
const wordFont = {
  g: "Font",
  rows: [
    [...FONT.rows[0], it("home.highlight", "highlight", "Highlight", "Text highlight")],
    FONT.rows[1],
    FONT.rows[2],
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
export function tabsFor(format, unit, edit) {
  const viewOnly = { Home: [{ g: "Clipboard", big: [copy] }, nav(unit), find] };
  const officeHome = [{ g: "Clipboard", big: [copy] }, FONT, nav(unit), find];
  const tabs =
    format === "xlsx" && edit
      ? { Home: [...XLSX_HOME.slice(0, 5), nav(unit), XLSX_HOME[5]] }
      : format === "pptx" && edit
        ? {
            Home: [...officeHome.slice(0, 2), lists, drawing, ...officeHome.slice(2)],
            Insert: [
              {
                g: "Slides",
                big: [
                  it("insert.slide", "slide", "New Slide"),
                  it("insert.duplicate-slide", "duplicate", "Duplicate Slide"),
                  it("insert.move-slide-up", "chevUp", "Move Slide Up"),
                  it("insert.move-slide-down", "chevDown", "Move Slide Down"),
                  it("insert.text-box", "text", "Text Box"),
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
