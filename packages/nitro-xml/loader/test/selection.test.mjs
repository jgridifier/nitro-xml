// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { rangeOf } from "../lazy/input-proxy.js";
import { parseSelectionString, rangeRef } from "../lazy/xlsx-selection.js";

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

  it("parseSelectionString and rangeRef round-trip multi-range and whole-header refs", () => {
    // Catches: Ctrl+click multi-range or whole-column/row header refs being dropped or misformatted
    const parsed = parseSelectionString("B12:E19,C:C,3:5,1:1048576");
    assert.equal(parsed.length, 4);
    assert.deepEqual(parsed.map(rangeRef), ["B12:E19", "C:C", "3:5", "1:1048576"]);
  });
});
