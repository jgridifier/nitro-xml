// SPDX-License-Identifier: MIT
// Ribbon manifests for the Office-style frame. A control is listed only if it maps to a real
// action: a viewer `shellAction` op (o) or an editor ribbon command id (c, sent as shellAction("cmd", id)).
// Every `c` id below was checked against the engine (the saved bytes change); anything the editor
// registers but does not implement (cut, paste, fill, borders, bullets) is deliberately absent.
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
const copy = { i: "copy", l: "Copy", t: "Copy (Ctrl+C)", o: "copy" };

const FONT = {
  g: "Font",
  rows: [
    [
      it("home.bold", "bold", "Bold", "Bold (Ctrl+B)"),
      it("home.italic", "italic", "Italic", "Italic (Ctrl+I)"),
      it("home.underline", "underline", "Underline", "Underline (Ctrl+U)"),
    ],
    [
      it("home.align-left", "left", "Align left"),
      it("home.align-center", "center", "Center"),
      it("home.align-right", "right", "Align right"),
      it("home.font-size", "grow", "Font size", "Set the font size"),
      it("home.font-color", "color", "Font color", "Set the font color"),
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
export function tabsFor(format, unit, edit) {
  const viewOnly = { Home: [{ g: "Clipboard", big: [copy] }, nav(unit), find] };
  const officeHome = [{ g: "Clipboard", big: [copy] }, FONT, nav(unit), find];
  const tabs =
    format === "xlsx" && edit
      ? { Home: [...XLSX_HOME.slice(0, 5), nav(unit), XLSX_HOME[5]] }
      : format === "pptx" && edit
        ? {
            Home: officeHome,
            Insert: [
              {
                g: "Slides",
                big: [
                  it("insert.slide", "slide", "New Slide"),
                  it("insert.duplicate-slide", "duplicate", "Duplicate Slide"),
                  it("insert.move-slide-up", "chevUp", "Move Slide Up"),
                  it("insert.move-slide-down", "chevDown", "Move Slide Down"),
                  it("insert.text-box", "text", "Text Box"),
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
              Home: officeHome,
              Insert: [
                {
                  g: "Tables",
                  big: [
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
