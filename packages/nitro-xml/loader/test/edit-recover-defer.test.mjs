// SPDX-License-Identifier: MIT
import { describe, it, before } from "node:test";
import assert from "node:assert/strict";

// element.js extends HTMLElement; Node unit tests have no DOM — stub a base.
before(() => {
  if (typeof globalThis.HTMLElement === "undefined") {
    globalThis.HTMLElement = class HTMLElement {};
  }
  if (typeof globalThis.customElements === "undefined") {
    globalThis.customElements = { get() {}, define() {} };
  }
});

describe("edit recover with deferred journal (#445)", () => {
  it("attempts recoverEditOnce when _journal is still null but originals exist", async () => {
    // Catches: #445 defer-journal left _journal null until first edit, while
    // ViewerElement._fail required _journal — so worker_crashed before the first
    // edit skipped recoverEditOnce and showed a hard error instead.
    const { ViewerElement } = await import("../element.js");
    let recovered = 0;
    let shown = 0;
    const el = Object.create(ViewerElement.prototype);
    el._readonly = false;
    el._d = {
      _journal: null,
      _originalBytes: new Uint8Array([1, 2, 3]),
      _editRecoveryAttempted: false,
      async recoverEditOnce() {
        recovered++;
        return this;
      },
    };
    el._clr = () => {};
    el._ao = async () => {};
    el._tick = () => {};
    el._showViewerError = () => {
      shown++;
    };

    el._fail({ code: "worker_crashed" });
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(recovered, 1);
    assert.equal(shown, 0);
  });

  it("surfaces an error when recoverEditOnce cannot fetch originals", async () => {
    // Catches: edit-mode crash with no WASM snapshot still must not silently succeed.
    const { ViewerElement } = await import("../element.js");
    let recovered = 0;
    let shown = 0;
    const el = Object.create(ViewerElement.prototype);
    el._readonly = false;
    el._d = {
      _journal: null,
      _originalBytes: null,
      _editRecoveryAttempted: false,
      async recoverEditOnce() {
        recovered++;
        throw Object.assign(new Error("no bytes"), { code: "editor_recovery_failed" });
      },
    };
    el._clr = () => {};
    el._ao = async () => {};
    el._tick = () => {};
    el._showViewerError = () => {
      shown++;
    };

    el._fail({ code: "worker_crashed" });
    await new Promise((r) => setTimeout(r, 0));
    assert.equal(recovered, 1);
    assert.equal(shown, 1);
  });
});
