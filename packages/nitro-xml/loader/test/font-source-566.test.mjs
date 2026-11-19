// SPDX-License-Identifier: MIT
// Catches: WorkerDocument function fontSource falling back to Google default (#566).
//          fontSource:"page" sending non-serializable to worker.
//          fonts array being ignored for WorkerDocument.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { setDefaultFontSource, getDefaultFontSource, _fd } from "../font-source.js";

describe("font-source #566", () => {
  it("_fd.f round-trips the fonts array (default fonts store for initCore)", () => {
    // Catches: initCore({fonts}) not storing the array for WorkerDocument to pick up.
    const list = [{ family: "TestFont", url: "local(monospace)", weight: "400", style: "normal" }];
    _fd.f = list;
    assert.deepEqual(_fd.f, list);
    _fd.f = undefined;
    assert.equal(_fd.f, undefined);
  });

  it("setDefaultFontSource accepts 'page' and 'off' without throwing", () => {
    // Catches: fontSource:"page" being rejected or mishandled at the storage level.
    setDefaultFontSource("page");
    assert.equal(getDefaultFontSource(), "page");
    setDefaultFontSource("off");
    assert.equal(getDefaultFontSource(), "off");
    setDefaultFontSource(undefined);
    assert.equal(getDefaultFontSource(), undefined);
  });
});
