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
