// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { lightMutResult, chromeFromResult } from "../lazy/frame-build.js";

describe("mut-frame result lightening", () => {
  it("keeps copy text and promotes chrome onto the frame path", () => {
    // Catches: lightMutResult dropping shellAction `c`/`h` so pptx shape-copy
    // clipboard is empty and embedded Find/Next never see chrome updates (#351).
    const raw = JSON.stringify({
      h: "<div class=\"f\">find</div>",
      c: "Hello shape",
      copyText: "Hello shape",
      p: null,
      ui: { chromeHtml: "<div class=\"f\">find</div>", find: { open: true } },
      save: false,
    });
    const light = JSON.parse(lightMutResult(raw));
    assert.equal(light.c, "Hello shape");
    assert.equal(light.p, null);
    assert.equal(light.h, undefined);
    assert.equal(light.ui, undefined);
    assert.deepEqual(chromeFromResult(raw), { h: "<div class=\"f\">find</div>" });
  });
});
