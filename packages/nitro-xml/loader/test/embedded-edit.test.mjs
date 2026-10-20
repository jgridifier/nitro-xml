// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";
import { MANIFEST } from "../lazy/embedded/manifest.js";

const pptxRibbon = readFileSync(
  fileURLToPath(new URL("../../../../rs/crates/nitro-xml-wasm/src/pptx_edit.rs", import.meta.url)),
  "utf8",
);
const docxRibbon = readFileSync(
  fileURLToPath(new URL("../../../../rs/crates/nitro-xml-wasm/src/docx_edit.rs", import.meta.url)),
  "utf8",
);

const cmds = (format) => MANIFEST[format].groups.flatMap((g) => g.items).map((it) => it.c).filter(Boolean);

describe("embedded pptx and docx toolbars", () => {
  it("shows only commands the PowerPoint editor ribbon emits", () => {
    // Catches: an embedded PowerPoint button for a command the editor does not implement.
    const bound = cmds("pptx");
    assert.ok(bound.length >= 12);
    for (const id of bound) assert.ok(pptxRibbon.includes(`id: "${id}"`), `${id} is not on the pptx ribbon`);
    for (const id of ["file.save", "edit.undo", "edit.redo", "home.bold", "home.font-size", "home.font-color", "insert.slide", "insert.duplicate-slide", "insert.text-box", "home.delete-slide"]) {
      assert.ok(bound.includes(id), `missing embedded pptx command ${id}`);
    }
    assert.ok(!bound.includes("insert.table"), "pptx table insert is not on this engine ribbon");
    assert.ok(!bound.includes("insert.move-slide-up"));
  });

  it("shows only commands the Word editor ribbon emits", () => {
    // Catches: an embedded Word button for a slide command, or a table command the editor does not write.
    const bound = cmds("docx");
    assert.ok(bound.length >= 12);
    for (const id of bound) assert.ok(docxRibbon.includes(`id: "${id}"`), `${id} is not on the docx ribbon`);
    for (const id of ["file.save", "edit.undo", "home.bold", "home.font-color", "insert.table", "table.insert-row", "table.delete-row", "table.insert-column", "table.delete-column"]) {
      assert.ok(bound.includes(id), `missing embedded docx command ${id}`);
    }
    assert.ok(!bound.includes("insert.slide"));
    assert.ok(!bound.includes("table.delete"), "whole-table delete is not on this engine ribbon");
  });
});
