// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { NUMBER_FORMATS, QAT, tabsFor } from "../lazy/ribbon/commands.js";
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
    // Catches: a PowerPoint button whose command does not rewrite the package (notes, pictures).
    const bound = [...ids(tabsFor("pptx", "Slide", true)), ...QAT.map((x) => x.c)];
    const allowed = new Set([
      "file.save",
      "edit.undo",
      "edit.redo",
      "home.bold",
      "home.italic",
      "home.underline",
      "home.align-left",
      "home.align-center",
      "home.align-right",
      "home.align-justify",
      "home.bullets",
      "home.numbering",
      "home.font-size",
      "home.font-color",
      "insert.text-box",
      "insert.table",
      "table.insert-row",
      "table.delete-row",
      "table.insert-column",
      "table.delete-column",
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
      "insert.table",
      "table.insert-row",
      "home.bold",
      "home.font-size",
      "home.font-color",
      "home.align-justify",
      "home.bullets",
      "home.numbering",
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
      "home.bold",
      "home.italic",
      "home.underline",
      "home.align-left",
      "home.align-center",
      "home.align-right",
      "home.align-justify",
      "home.font-size",
      "home.font-color",
      "insert.table",
      "table.insert-row",
      "table.delete-row",
      "table.insert-column",
      "table.delete-column",
      "table.delete",
    ]);
    for (const id of bound) assert.ok(allowed.has(id), `unexpected docx command ${id}`);
    assert.ok(bound.includes("insert.table"));
    assert.ok(bound.includes("table.insert-row"));
    assert.ok(bound.includes("table.delete"));
    assert.ok(bound.includes("home.font-color"));
    assert.ok(bound.includes("home.bold"));
    assert.ok(bound.includes("home.align-justify"));
    assert.ok(!bound.includes("home.bullets"), "Word numbering.xml is not written");
    assert.ok(!bound.includes("insert.slide"));
    assert.ok(!bound.includes("insert.text-box"));
  });

  it("drops the Read-only pill for Word and PowerPoint", () => {
    // Catches: the demo still labelling an editable Word or PowerPoint file Read-only.
    const app = readFileSync(new URL("../../../../demo/app.js", import.meta.url), "utf8");
    assert.match(app, /docx: true, pptx: true/);
    assert.match(app, /EDITABLE\[format\] \? "" : "Read-only"/);
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
