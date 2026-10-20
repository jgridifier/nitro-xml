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
    for (const id of ["file.save", "edit.undo", "edit.redo", "home.bold", "home.strikethrough", "home.font-size", "home.font-color", "home.font-name", "home.indent", "home.outdent", "home.line-spacing", "home.subscript", "home.superscript", "home.align-justify", "home.bullets", "home.numbering", "insert.slide", "insert.duplicate-slide", "insert.move-slide-up", "insert.move-slide-down", "insert.text-box", "insert.picture", "home.delete-slide", "insert.table", "table.insert-row", "table.delete-column"]) {
      assert.ok(bound.includes(id), `missing embedded pptx command ${id}`);
    }
  });

  it("shows only commands the Word editor ribbon emits", () => {
    // Catches: an embedded Word button for a slide command, or a table command the editor does not write.
    const bound = cmds("docx");
    assert.ok(bound.length >= 12);
    for (const id of bound) assert.ok(docxRibbon.includes(`id: "${id}"`), `${id} is not on the docx ribbon`);
    for (const id of ["file.save", "edit.undo", "home.bold", "home.strikethrough", "home.highlight", "home.font-color", "home.font-name", "home.indent", "home.outdent", "home.line-spacing", "home.subscript", "home.superscript", "home.align-justify", "home.numbering", "home.bullets", "insert.picture", "insert.table", "table.insert-row", "table.delete-row", "table.insert-column", "table.delete-column", "table.delete"]) {
      assert.ok(bound.includes(id), `missing embedded docx command ${id}`);
    }
    assert.ok(!bound.includes("insert.slide"));
  });
});
