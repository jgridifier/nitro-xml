// SPDX-License-Identifier: MIT
// Catches: M1 known-loss allowlist must keep xlsx S + docx S/M peak_mem as
// non-blocking LOSS while every other LOSS still blocks enforce readiness.
"use strict";

const assert = require("node:assert/strict");
const { describe, it } = require("node:test");
const {
  KNOWN_LOSS_CELLS,
  KNOWN_LOSS_DECISION,
  classifyMeasuredLoss,
  isKnownLossCell,
} = require("./perf-beats-known-loss.cjs");

describe("perf-beats known-loss allowlist", () => {
  it("documents Jared's #30 compile-floor decision URL", () => {
    assert.match(KNOWN_LOSS_DECISION, /issuecomment-6341154384/);
  });

  it("allowlists exactly xlsx S and docx S/M peak_mem", () => {
    assert.deepEqual(
      KNOWN_LOSS_CELLS.map((c) => `${c.format}|${c.tier}|${c.metric}`),
      ["xlsx|S|peak_mem", "docx|S|peak_mem", "docx|M|peak_mem"],
    );
  });

  it("keeps known-loss peak_mem cells non-blocking while still LOSS", () => {
    for (const c of KNOWN_LOSS_CELLS) {
      assert.equal(isKnownLossCell(c.format, c.tier, c.metric), true);
      const cls = classifyMeasuredLoss(c.format, c.tier, c.metric, "LOSS");
      assert.equal(cls.knownLoss, true);
      assert.equal(cls.blocking, false);
      assert.match(cls.label, /peak_mem LOSS$/);
    }
  });

  it("still blocks any other LOSS cell (openDocument or other peak_mem)", () => {
    const cases = [
      ["xlsx", "S", "openDocument"],
      ["xlsx", "M", "peak_mem"],
      ["docx", "L", "peak_mem"],
      ["pptx", "S", "peak_mem"],
      ["docx", "S", "openDocument"],
    ];
    for (const [format, tier, metric] of cases) {
      assert.equal(isKnownLossCell(format, tier, metric), false);
      const cls = classifyMeasuredLoss(format, tier, metric, "LOSS");
      assert.equal(cls.knownLoss, false);
      assert.equal(cls.blocking, true);
    }
  });
});
