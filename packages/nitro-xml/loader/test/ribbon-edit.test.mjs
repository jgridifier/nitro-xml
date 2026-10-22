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
      "home.indent",
      "home.outdent",
      "home.line-spacing",
      "home.subscript",
      "home.superscript",
      "home.font-name",
      "home.shape-fill",
      "home.shape-outline",
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
      "home.space-before",
      "home.space-after",
      "home.hanging-indent",
      "home.first-line",
      "home.indent-right",
      "home.outdent-right",
      "home.double-underline",
      "home.caps",
      "home.expand-spacing",
      "home.condense-spacing",
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
    assert.ok(bound.includes("home.highlight"));
    assert.ok(bound.includes("home.bullets"));
    assert.ok(bound.includes("home.indent"));
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
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.caps"));
    assert.ok(bound.includes("home.expand-spacing"));
    assert.ok(bound.includes("home.condense-spacing"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.expand-spacing"));
    assert.ok(!ids(tabsFor("pptx", "Slide", true)).includes("home.condense-spacing"));
    assert.ok(bound.includes("home.font-name"));
    assert.ok(bound.includes("home.style-heading-1"));
    assert.ok(bound.includes("insert.page-break"));
    assert.ok(!bound.includes("insert.slide"));
    assert.ok(!bound.includes("insert.text-box"));
    assert.ok(!bound.includes("home.group"));
    assert.ok(!bound.includes("home.ungroup"));
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
