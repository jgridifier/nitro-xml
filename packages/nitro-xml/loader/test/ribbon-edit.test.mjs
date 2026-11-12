// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { NUMBER_FORMATS, QAT, officeCommandHidden, tabsFor } from "../lazy/ribbon/commands.js";
import { MANIFEST } from "../lazy/embedded/manifest.js";
import { trackDirty } from "../lazy/ribbon/dirty.js";

const enginePath = fileURLToPath(new URL("../../../../rs/crates/nitro-xml-xlsx/src/editor/commands.rs", import.meta.url));
const engine = readFileSync(enginePath, "utf8");
const ids = (tabs) =>
  Object.values(tabs)
    .flat()
    .flatMap((g) => [...g.big, ...g.rows.flat()])
    .flatMap((x) => [x, ...(x.m || [])])
    .map((x) => x.c)
    .filter(Boolean);

describe("Office ribbon manifests", () => {
  it("only binds editor commands the xlsx engine maps", () => {
    // Catches: a ribbon button wired to an id the engine leaves Unmapped (a dead control that looks live).
    const bound = [...ids(tabsFor("xlsx", "Sheet", true)), ...QAT.map((x) => x.c), ...NUMBER_FORMATS.map((x) => x.c)];
    assert.ok(bound.length > 20);
    for (const id of bound) assert.ok(engine.includes(`"${id}"`), `${id} is not mapped in nitro-xml-xlsx editor/commands.rs`);
  });

  it("shows no editing commands where the format or element is read-only", () => {
    // Catches: pptx/docx demos listing Excel edit commands because `readonly="false"` was flipped on.
    for (const f of ["pptx", "docx"]) assert.deepEqual(ids(tabsFor(f, "Slide", false)), []);
    assert.deepEqual(ids(tabsFor("xlsx", "Sheet", false)), []);
  });

  it("lists only PowerPoint commands that change the saved package", () => {
    // Catches: a PowerPoint button whose command does not rewrite the package (notes).
    const bound = [...ids(tabsFor("pptx", "Slide", true)), ...QAT.map((x) => x.c)];
    const allowed = new Set([
      "file.save",
      "edit.undo",
      "edit.redo",
      "home.cut",
      "home.copy",
      "home.paste",
      "home.bold",
      "home.italic",
      "home.underline",
      "home.strikethrough",
      "home.double-strike",
      "home.small-caps",
      "home.caps",
      "home.align-left",
      "home.align-center",
      "home.align-right",
      "home.align-justify",
      "home.bullets",
      "home.numbering",
      "home.font-size",
      "home.grow-font",
      "home.shrink-font",
      "home.font-color",
      "home.font-name",
      "home.indent",
      "home.outdent",
      "home.line-spacing",
      "home.subscript",
      "home.superscript",
      "home.shape-fill",
      "home.shape-outline",
      "home.shape-dash",
      "home.layout",
      "insert.text-box",
      "insert.shape",
      "insert.picture",
      "home.duplicate",
      "home.delete",
      "home.bring-forward",
      "home.send-backward",
      "home.bring-to-front",
      "home.send-to-back",
      "home.group",
      "home.ungroup",
      "insert.table",
      "table.insert-row",
      "table.delete-row",
      "table.insert-column",
      "table.delete-column",
      "table.cell-text",
      "table.delete",
      "insert.slide",
      "insert.duplicate-slide",
      "insert.move-slide-up",
      "insert.move-slide-down",
      "home.delete-slide",
    ]);
    for (const id of bound) assert.ok(allowed.has(id), `unexpected pptx command ${id}`);
    for (const id of [
      "insert.slide",
      "insert.duplicate-slide",
      "insert.move-slide-down",
      "home.delete-slide",
      "insert.text-box",
      "insert.shape",
      "home.duplicate",
      "home.delete",
      "home.bring-forward",
      "home.send-backward",
      "home.bring-to-front",
      "home.send-to-back",
      "insert.picture",
      "insert.table",
      "table.insert-row",
      "table.cell-text",
      "table.delete",
      "home.cut",
      "home.copy",
      "home.paste",
      "home.bold",
      "home.font-size",
      "home.grow-font",
      "home.shrink-font",
      "home.font-color",
      "home.align-justify",
      "home.bullets",
      "home.numbering",
      "home.strikethrough",
      "home.double-strike",
      "home.small-caps",
      "home.caps",
      "home.indent",
      "home.outdent",
      "home.line-spacing",
      "home.subscript",
      "home.superscript",
      "home.font-name",
      "home.shape-fill",
      "home.shape-outline",
      "home.shape-dash",
      "home.layout",
      "home.group",
      "home.ungroup",
    ]) {
      assert.ok(bound.includes(id), `missing pptx command ${id}`);
    }
  });

  it("lists only Word commands that change the saved package", () => {
    // Catches: Word advertising a slide command, or omitting the table insert that writes w:tbl.
    const bound = [...ids(tabsFor("docx", "Page", true)), ...QAT.map((x) => x.c)];
    const allowed = new Set([
      "file.save",
      "edit.undo",
      "edit.redo",
      "home.cut",
      "home.copy",
      "home.paste",
      "home.bold",
      "home.italic",
      "home.underline",
      "home.strikethrough",
      "home.highlight",
      "home.align-left",
      "home.align-center",
      "home.align-right",
      "home.align-justify",
      "home.numbering",
      "home.style-normal",
      "home.style-title",
      "home.style-heading-1",
      "home.style-heading-2",
      "home.style-heading-3",
      "insert.page-break",
      "home.font-size",
      "home.grow-font",
      "home.shrink-font",
      "home.font-color",
      "home.font-name",
      "home.indent",
      "home.outdent",
      "home.line-spacing",
      "home.shading",
      "home.border-bottom",
      "home.border-top",
      "home.border-left",
      "home.border-right",
      "home.border-double",
      "home.border-dash",
      "home.border-dot",
      "home.landscape",
      "home.narrow-margins",
      "home.moderate-margins",
      "home.wide-margins",
      "home.page-a4",
      "home.page-legal",
      "home.page-break-before",
      "home.space-before",
      "home.space-after",
      "home.hanging-indent",
      "home.first-line",
      "home.indent-right",
      "home.outdent-right",
      "home.double-underline",
      "home.caps",
      "home.small-caps",
      "home.double-strike",
      "home.hidden",
      "home.expand-spacing",
      "home.condense-spacing",
      "home.scale-up",
      "home.scale-down",
      "home.subscript",
      "home.superscript",
      "home.bullets",
      "insert.picture",
      "insert.table",
      "table.insert-row",
      "table.delete-row",
      "table.insert-column",
      "table.delete-column",
      "table.cell-text",
      "table.delete",
    ]);
    for (const id of bound) assert.ok(allowed.has(id), `unexpected docx command ${id}`);
    assert.ok(bound.includes("insert.picture"));
    assert.ok(bound.includes("insert.table"));
    assert.ok(bound.includes("table.insert-row"));
    assert.ok(bound.includes("table.cell-text"));
    assert.ok(bound.includes("table.delete"));
    assert.ok(bound.includes("home.font-color"));
    assert.ok(bound.includes("home.cut"));
    assert.ok(bound.includes("home.copy"));
    assert.ok(bound.includes("home.paste"));
    assert.ok(bound.includes("home.bold"));
    assert.ok(bound.includes("home.align-justify"));
    assert.ok(bound.includes("home.numbering"));
    assert.ok(bound.includes("home.strikethrough"));
    assert.ok(ids(tabsFor("pptx", "Slide", true)).includes("home.small-caps"));
    assert.ok(bound.includes("home.highlight"));
    assert.ok(bound.includes("home.bullets"));
    assert.ok(bound.includes("home.indent"));
    assert.ok(bound.includes("home.shading"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.shading"));
    assert.ok(bound.includes("home.border-bottom"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-bottom"));
    assert.ok(bound.includes("home.border-top"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-top"));
    assert.ok(bound.includes("home.border-left"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-left"));
    assert.ok(bound.includes("home.border-right"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-right"));
    assert.ok(bound.includes("home.border-double"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-double"));
    assert.ok(bound.includes("home.border-dash"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-dash"));
    assert.ok(bound.includes("home.border-dot"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.border-dot"));
    assert.ok(bound.includes("home.landscape"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.landscape"));
    assert.ok(bound.includes("home.narrow-margins"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.narrow-margins"));
    assert.ok(bound.includes("home.moderate-margins"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.moderate-margins"));
    assert.ok(bound.includes("home.wide-margins"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.wide-margins"));
    assert.ok(bound.includes("home.page-a4"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.page-a4"));
    assert.ok(bound.includes("home.page-legal"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.page-legal"));
    assert.ok(bound.includes("home.page-break-before"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.page-break-before"));
    assert.ok(bound.includes("home.space-before"));
    assert.ok(bound.includes("home.space-after"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.space-before"));
    assert.ok(bound.includes("home.hanging-indent"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.hanging-indent"));
    assert.ok(bound.includes("home.first-line"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.first-line"));
    assert.ok(bound.includes("home.indent-right"));
    assert.ok(bound.includes("home.outdent-right"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.indent-right"));
    assert.ok(bound.includes("home.double-underline"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.double-underline"));
    assert.ok(bound.includes("home.caps"));
    assert.ok(ids(tabsFor("pptx", "Slide", true)).includes("home.caps"));
    assert.ok(!bound.includes("home.shape-dash"));
    assert.ok(ids(tabsFor("pptx", "Slide", true)).includes("home.shape-dash"));
    assert.ok(bound.includes("home.small-caps"));
    assert.ok(bound.includes("home.double-strike"));
    assert.ok(ids(tabsFor("pptx", "Slide", true)).includes("home.double-strike"));
    assert.ok(ids(tabsFor("pptx", "Slide", true)).includes("home.small-caps"));
    assert.ok(bound.includes("home.hidden"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.hidden"));
    assert.ok(bound.includes("home.expand-spacing"));
    assert.ok(bound.includes("home.condense-spacing"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.expand-spacing"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.condense-spacing"));
    assert.ok(bound.includes("home.scale-up"));
    assert.ok(bound.includes("home.scale-down"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.scale-up"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.scale-down"));
    assert.ok(bound.includes("home.font-name"));
    assert.ok(bound.includes("home.style-heading-1"));
    assert.ok(bound.includes("insert.page-break"));
    assert.ok(!bound.includes("insert.slide"));
    assert.ok(!bound.includes("insert.text-box"));
    assert.ok(!bound.includes("home.group"));
    assert.ok(!bound.includes("home.ungroup"));
  });

  it("Word ribbon groups match Office conventions: Font is char-only, Picture is own group, Layout tab holds page-setup, nav is on View", () => {
    // Catches: landscape/margin/page-size in Font group; insert.picture buried in Tables; Layout tab missing; prev/next page on Home instead of View.
    const tabs = tabsFor("docx", "Page", true);
    const flatten = (g) => [...g.big, ...g.rows.flat()].flatMap((x) => [x, ...(x.m || [])]);
    const groupCmds = (g) => new Set(flatten(g).map((x) => x.c).filter(Boolean));
    const groupOps = (g) => new Set(flatten(g).map((x) => x.o).filter(Boolean));
    const tabCmds = (t) => new Set((tabs[t] || []).flatMap((g) => flatten(g)).map((x) => x.c).filter(Boolean));
    const tabOps = (t) => new Set((tabs[t] || []).flatMap((g) => flatten(g)).map((x) => x.o).filter(Boolean));

    // Font group: character formatting only — no alignment, no paragraph props, no page-setup.
    const fontGroup = tabs.Home.find((g) => g.g === "Font");
    assert.ok(fontGroup, "Home tab must have a Font group");
    const fontCmds = groupCmds(fontGroup);
    for (const banned of [
      "home.landscape", "home.narrow-margins", "home.moderate-margins", "home.wide-margins",
      "home.page-a4", "home.page-legal", "home.page-break-before",
      "home.align-left", "home.align-center", "home.align-right", "home.align-justify",
      "home.bullets", "home.numbering", "home.indent", "home.outdent", "home.shading",
    ]) {
      assert.ok(!fontCmds.has(banned), `Font group must not contain ${banned}`);
    }

    // Home tab group order: Clipboard, Font, Paragraph, Styles, Editing (no navigation group).
    assert.deepEqual(tabs.Home.map((g) => g.g), ["Clipboard", "Font", "Paragraph", "Styles", "Editing"]);

    // insert.picture must be in its own group named "Pictures", not in Tables.
    const pictureGroup = tabs.Insert.find((g) => g.big.some((x) => x.c === "insert.picture"));
    assert.ok(pictureGroup, "insert.picture must exist on Insert tab");
    assert.equal(pictureGroup.g, "Pictures", "insert.picture must be in Pictures group, not Tables");
    const tablesGroup = tabs.Insert.find((g) => g.g === "Tables");
    assert.ok(tablesGroup, "Insert tab must have a Tables group");
    assert.ok(!tablesGroup.big.some((x) => x.c === "insert.picture"), "insert.picture must not be in Tables group");

    // Insert tab must have a Pages group with insert.page-break.
    const pagesInsertGroup = tabs.Insert.find((g) => g.g === "Pages");
    assert.ok(pagesInsertGroup, "Insert tab must have a Pages group");
    assert.ok(pagesInsertGroup.big.some((x) => x.c === "insert.page-break"), "Pages group must contain insert.page-break");

    // Layout tab exists and contains all page-setup commands.
    assert.ok(tabs.Layout, "Layout tab must exist for docx");
    const layoutCmds = tabCmds("Layout");
    for (const pg of [
      "home.landscape", "home.narrow-margins", "home.moderate-margins", "home.wide-margins",
      "home.page-a4", "home.page-legal", "home.page-break-before",
    ]) {
      assert.ok(layoutCmds.has(pg), `Layout tab must contain ${pg}`);
    }

    // Navigation (prev/next page) must be on View tab, not Home tab.
    const homeOps = tabOps("Home");
    assert.ok(!homeOps.has("nav-prev"), "nav-prev must not be on Home tab for docx");
    assert.ok(!homeOps.has("nav-next"), "nav-next must not be on Home tab for docx");
    const viewOps = tabOps("View");
    assert.ok(viewOps.has("nav-prev"), "nav-prev must be on View tab for docx");
    assert.ok(viewOps.has("nav-next"), "nav-next must be on View tab for docx");

    // pptx must NOT have a Layout tab (page-setup is Word-only).
    assert.ok(!tabsFor("pptx", "Slide", true).Layout, "pptx must not have a Layout tab");
  });


  it("Word and PowerPoint Font and Paragraph groups use at most two rows, with less-common commands in dropdowns", () => {
    // Catches: a third Font or Paragraph row sneaking back, or effects/borders left as a third row instead of a menu.
    for (const [format, unit] of [["docx", "Page"], ["pptx", "Slide"]]) {
      const tabs = tabsFor(format, unit, true);
      for (const name of ["Font", "Paragraph"]) {
        const g = tabs.Home.find((x) => x.g === name);
        assert.ok(g, `${format} Home must have a ${name} group`);
        assert.ok(g.rows.length <= 2, `${format} ${name} has ${g.rows.length} rows, max is 2`);
      }
    }
    const word = tabsFor("docx", "Page", true);
    const wordFont = word.Home.find((g) => g.g === "Font");
    const wordPara = word.Home.find((g) => g.g === "Paragraph");
    const inMenu = (g, id) => [...g.big, ...g.rows.flat()].some((x) => (x.m || []).some((m) => m.c === id));
    for (const id of ["home.double-underline", "home.double-strike", "home.caps", "home.small-caps", "home.hidden", "home.expand-spacing", "home.scale-up"]) {
      assert.ok(inMenu(wordFont, id), `Word Font dropdown must contain ${id}`);
    }
    for (const id of ["home.line-spacing", "home.space-before", "home.hanging-indent", "home.shading", "home.border-bottom"]) {
      assert.ok(inMenu(wordPara, id), `Word Paragraph dropdown must contain ${id}`);
    }
    const xlsxFont = tabsFor("xlsx", "Sheet", true).Home.find((g) => g.g === "Font");
    const xlsxAlign = tabsFor("xlsx", "Sheet", true).Home.find((g) => g.g === "Alignment");
    assert.ok(xlsxFont.rows.length <= 2, "Excel Font must stay at most 2 rows");
    assert.ok(xlsxAlign.rows.length <= 2, "Excel Alignment must stay at most 2 rows");
  });

  it("PowerPoint Home follows Office-like groups: Clipboard, Slides, Font, Paragraph, Drawing, Editing; alignment is not in Font", () => {
    // Catches: PowerPoint Font still mixing alignment; New Slide left only on Insert; nav-prev left on Home; alignment missing from Paragraph.
    const tabs = tabsFor("pptx", "Slide", true);
    const names = tabs.Home.map((g) => g.g);
    for (const g of ["Clipboard", "Slides", "Font", "Paragraph", "Drawing", "Editing"]) {
      assert.ok(names.includes(g), `PowerPoint Home must have a ${g} group, got ${names.join(", ")}`);
    }
    const flatten = (g) => [...g.big, ...g.rows.flat()].flatMap((x) => [x, ...(x.m || [])]);
    const fontCmds = new Set(flatten(tabs.Home.find((g) => g.g === "Font")).map((x) => x.c).filter(Boolean));
    const paraCmds = new Set(flatten(tabs.Home.find((g) => g.g === "Paragraph")).map((x) => x.c).filter(Boolean));
    for (const a of ["home.align-left", "home.align-center", "home.align-right", "home.align-justify"]) {
      assert.ok(!fontCmds.has(a), `PowerPoint Font must not contain ${a}`);
      assert.ok(paraCmds.has(a), `PowerPoint Paragraph must contain ${a}`);
    }
    const homeCmds = new Set(tabs.Home.flatMap((g) => flatten(g)).map((x) => x.c).filter(Boolean));
    const insertCmds = new Set((tabs.Insert || []).flatMap((g) => flatten(g)).map((x) => x.c).filter(Boolean));
    const homeOps = new Set(tabs.Home.flatMap((g) => flatten(g)).map((x) => x.o).filter(Boolean));
    const viewOps = new Set((tabs.View || []).flatMap((g) => flatten(g)).map((x) => x.o).filter(Boolean));
    assert.ok(homeCmds.has("insert.slide"), "New Slide must be on PowerPoint Home");
    assert.ok(homeCmds.has("home.layout"), "Change layout must be on PowerPoint Home");
    assert.ok(insertCmds.has("insert.picture"), "Picture must stay on Insert");
    assert.ok(insertCmds.has("insert.shape"), "Shape must stay on Insert");
    assert.ok(insertCmds.has("insert.text-box"), "Text Box must stay on Insert");
    const pictures = tabs.Insert.find((g) => g.g === "Pictures");
    assert.ok(pictures && flatten(pictures).some((x) => x.c === "insert.picture"), "insert.picture must be in Pictures");
    assert.ok(!homeOps.has("nav-prev"), "nav-prev must not be on PowerPoint Home");
    assert.ok(viewOps.has("nav-prev"), "nav-prev must be on PowerPoint View");
    assert.ok(!tabs.Layout, "PowerPoint must not have a Word Layout tab");
  });

  it("drops the Read-only pill for Word and PowerPoint", () => {
    // Catches: the demo still labelling an editable Word or PowerPoint file Read-only.
    const app = readFileSync(new URL("../../../../demo/app.js", import.meta.url), "utf8");
    assert.match(app, /docx: true, pptx: true/);
    assert.match(app, /EDITABLE\[format\] \? "" : "Read-only"/);
  });

  it("hides a Pages command the WASM strip does not emit", () => {
    // Catches: a PowerPoint or Word ribbon button staying clickable after the editor drops that command.
    assert.equal(officeCommandHidden("pptx", true, false), true);
    assert.equal(officeCommandHidden("docx", true, false), true);
    assert.equal(officeCommandHidden("pptx", true, true), false);
    assert.equal(officeCommandHidden("docx", false, false), false);
    assert.equal(officeCommandHidden("xlsx", true, false), false);
    const frame = readFileSync(new URL("../lazy/ribbon/index.js", import.meta.url), "utf8");
    assert.match(frame, /b\.hidden = officeCommandHidden\(o\.format, !!ch\(\), !!m\)/);
  });

  it("keeps PowerPoint and Word ribbons inside the WASM strip", () => {
    // Catches: a GitHub Pages Group button on Word, or a PowerPoint Group button the strip does not emit.
    const strip = (rel) => {
      const src = readFileSync(new URL(rel, import.meta.url), "utf8");
      const start = src.indexOf("pub fn ribbon_html");
      const rest = src.slice(start + 1);
      const end = rest.search(/\nfn |\nimpl |\npub fn /);
      const body = src.slice(start, end === -1 ? src.length : start + 1 + end);
      return [...body.matchAll(/id: "([^"]+)"/g)].map((m) => m[1]);
    };
    const pptx = new Set(strip("../../../../rs/crates/nitro-xml-wasm/src/pptx_edit.rs"));
    const docx = new Set(strip("../../../../rs/crates/nitro-xml-wasm/src/docx_edit.rs"));
    const manifestIds = (format) =>
      MANIFEST[format].groups.flatMap((g) => g.items || []).map((x) => x.c).filter(Boolean);
    const off = [];
    for (const [format, rust, unit] of [
      ["pptx", pptx, "Slide"],
      ["docx", docx, "Page"],
    ]) {
      const bound = [...ids(tabsFor(format, unit, true)), ...QAT.map((x) => x.c), ...manifestIds(format)];
      for (const id of bound) assert.ok(rust.has(id), `${format} advertises ${id}, which the WASM strip does not emit`);
      for (const id of off) assert.ok(!bound.includes(id), `${format} still lists ${id}`);
      for (const id of ["file.save", "edit.undo", "edit.redo", "home.bold"]) assert.ok(rust.has(id));
    }
  });

  it("gives every control a unique key", () => {
    // Catches: two buttons sharing a data-k, so a click runs the wrong command.
    const tabs = tabsFor("xlsx", "Sheet", true);
    const keys = Object.values(tabs).flat().flatMap((g) => [...g.big, ...g.rows.flat()]).flatMap((x) => (x.r ? [] : [x, ...(x.m || [])])).map((x) => x.k);
    assert.equal(new Set(keys).size, keys.length);
  });
});

function fakeElement(initial) {
  const listeners = new Map();
  let bytes = initial;
  const el = {
    _d: {
      _worker: { postMessage() {} },
      _request: async () => ({ bytes }),
    },
    addEventListener: (e, f) => listeners.set(e, f),
    removeEventListener: (e) => listeners.delete(e),
    edit: (b) => (bytes = b),
    fire: (e) => listeners.get(e)(),
  };
  return el;
}

describe("unsaved-changes tracking", () => {
  it("goes dirty on a change, clean on undo back to the original, clean after save", async () => {
    // Catches: the unsaved dot sticking after Save, or never lighting for typed edits.
    const el = fakeElement(new Uint8Array([1, 2, 3]));
    const seen = [];
    const t = trackDirty(el, (d) => seen.push(d));
    await t.rebase();
    el.edit(new Uint8Array([1, 2, 4]));
    t.touch();
    await new Promise((r) => setTimeout(r, 450));
    assert.deepEqual(seen, [true]);
    t.saved(new Uint8Array([1, 2, 4]));
    assert.deepEqual(seen, [true, false]);
    t.touch();
    await new Promise((r) => setTimeout(r, 450));
    assert.deepEqual(seen, [true, false]);
    t.destroy();
  });

  it("stays dirty after a reopen that carried edits until saved", async () => {
    // Catches: switching Full/Embedded silently clearing the unsaved flag while the edits are still unsaved.
    const el = fakeElement(new Uint8Array([9]));
    const seen = [];
    const t = trackDirty(el, (d) => seen.push(d));
    await t.rebase();
    t.force();
    t.touch();
    await new Promise((r) => setTimeout(r, 450));
    assert.deepEqual(seen, [true]);
    t.destroy();
  });
});
