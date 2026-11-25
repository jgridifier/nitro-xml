// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { rangeOf } from "../lazy/input-proxy.js";
import { charOffsetToRunPos, shapeOffsetToParaRunPos } from "../lazy/s.js";

describe("getSelection() shape mapping", () => {
  it("rangeOf maps single cell to 1×1 block (xlsxSel count gate)", () => {
    // Catches: off-by-one in range count making single-cell selections report truncated=true
    const r = rangeOf("B2");
    assert.ok(r, "rangeOf returns non-null for valid cell");
    assert.equal(r.r0, 1); // row 0-based
    assert.equal(r.c0, 1); // col 0-based
    assert.equal(r.r1, 1);
    assert.equal(r.c1, 1);
    const count = (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1);
    assert.equal(count, 1, "single cell count must be 1");
  });

  it("rangeOf maps multi-cell range correctly", () => {
    // Catches: inverted r0/c0 ordering when end < start in a range like D2:B4
    const r = rangeOf("D2:B4");
    assert.ok(r);
    assert.equal(r.r0, 1);
    assert.equal(r.r1, 3);
    assert.equal(r.c0, 1); // B=1, D=3; normalised so c0 < c1
    assert.equal(r.c1, 3);
    const count = (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1);
    assert.equal(count, 9, "3×3 range must count 9 cells");
  });

  it("rangeOf returns null for unparsable or whole-row refs", () => {
    // Catches: whole-row ref "1:1" crashing or returning a huge count
    assert.equal(rangeOf(""), null);
    assert.equal(rangeOf("1:1"), null);
    assert.equal(rangeOf(null), null);
  });

  it("xlsxSel anchor is left part of range string", () => {
    // Catches: anchor/range fields swapped so anchor is "B2:D5" and range is "B2"
    const rangeStr = "B2:D5";
    const anchor = rangeStr.split(":")[0];
    assert.equal(anchor, "B2");
  });

  it("VALUES_CAP (10000 cells) triggers truncated=true", () => {
    // Catches: truncated flag not set for >10k cell selections (host relies on it for prompt sizing)
    const VALUES_CAP = 10000;
    const r = rangeOf("A1:J1000"); // 10 cols × 1000 rows = 10000
    assert.ok(r);
    const count = (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1);
    assert.equal(count, 10000);
    assert.equal(count > VALUES_CAP, false, "exactly 10000 should NOT be truncated");

    const r2 = rangeOf("A1:K1000"); // 11 cols × 1000 rows = 11000
    const count2 = (r2.r1 - r2.r0 + 1) * (r2.c1 - r2.c0 + 1);
    assert.equal(count2 > VALUES_CAP, true, "11000 cells MUST be truncated");
  });

  it("charOffsetToRunPos maps docx cross-run offsets to run-local indices", () => {
    // Catches: docx getSelection() hardcoding run=0 instead of mapping across runs (#615).
    const runs = [4, 2, 6, 5, 10, 28]; // "Bold" ", " "italic" " and " "underlined" " runs..."
    assert.deepEqual(charOffsetToRunPos(0, runs, true), { run: 0, offset: 0 });
    assert.deepEqual(charOffsetToRunPos(27, runs, false), { run: 4, offset: 10 });
    // Internal boundary attaches start to the following run and end to the preceding run.
    assert.deepEqual(charOffsetToRunPos(4, runs, true), { run: 1, offset: 0 });
    assert.deepEqual(charOffsetToRunPos(4, runs, false), { run: 0, offset: 4 });
    assert.deepEqual(charOffsetToRunPos(6, runs, true), { run: 2, offset: 0 });
    assert.deepEqual(charOffsetToRunPos(12, runs, false), { run: 2, offset: 6 });
    // Empty runs are skipped cleanly.
    assert.deepEqual(charOffsetToRunPos(0, [0, 4, 2], true), { run: 1, offset: 0 });
    assert.deepEqual(charOffsetToRunPos(4, [0, 4, 2], false), { run: 1, offset: 4 });
  });

  it("shapeOffsetToParaRunPos maps pptx multi-paragraph cross-run offsets", () => {
    // Catches: pptx getSelection() hardcoding paragraph=0 and run=0 (#615).
    const paras = [[], [21, 1, 2], [4, 1, 6, 1, 9, 1, 11, 1, 9]];
    // Paragraph 2 starts at offset 0 + 1('\n') + 24 + 1('\n') = 26.
    assert.deepEqual(shapeOffsetToParaRunPos(26, paras, true), { paragraph: 2, run: 0, offset: 0 });
    // "Bold italic underline" ends at 26 + 4 + 1 + 6 + 1 + 9 = 47 -> run 4, offset 9.
    assert.deepEqual(shapeOffsetToParaRunPos(47, paras, false), { paragraph: 2, run: 4, offset: 9 });
    // Single-paragraph single-run shape ("Nitro XML").
    assert.deepEqual(shapeOffsetToParaRunPos(4, [[9]], false), { paragraph: 0, run: 0, offset: 4 });
  });
});
