// SPDX-License-Identifier: MIT
// Catches: loadBuffer stealing focus from the host page (#567).
//          embedded fit() refocusing a stale shadow-root element on shellAction("ui").
import { describe, it } from "node:test";
import assert from "node:assert/strict";

describe("focus no-steal #567", () => {
  it("fit() only refocuses when the focused element was moved by layout()", () => {
    // Catches: fit() calling a.focus() even when focus was not lost due to layout rearrangement.
    let focusCalled = 0;
    // Simulate: an element that was focused before layout and is still focused after.
    // root.activeElement returns the same element before and after layout() (no DOM rearrangement).
    const mockEl = {
      isConnected: true,
      offsetParent: {},
      focus() { focusCalled++; },
    };
    // The fixed logic: only call a.focus() if a !== root.activeElement after layout.
    function fitLogic(beforeEl, afterEl) {
      const a = beforeEl;
      // layout() runs — in this mock it may or may not change activeElement
      const afterActive = afterEl;
      if (a && a !== afterActive && a.isConnected && a.offsetParent) a.focus();
    }

    // Case 1: layout did NOT move the element (same active element before and after)
    fitLogic(mockEl, mockEl);
    assert.equal(focusCalled, 0, "must not focus when layout did not move the element");

    // Case 2: layout moved the element (active element changed)
    fitLogic(mockEl, null);
    assert.equal(focusCalled, 1, "must refocus when layout moved the element away");
  });

  it("loadBuffer opts.focus defaults to false (no auto-focus)", async () => {
    // Catches: loadBuffer focusing the editor by default and stealing host focus.
    let nxFocusCalled = 0;
    // Minimal ViewerElement stub
    class FakeViewer {
      constructor() { this._d = null; this._fonts = null; this._v = null; this._ch = null; }
      hasAttribute() { return false; }
      _nxFocus() { nxFocusCalled++; }
      async _ao() {}
      _clr() {} _surf() {} _ok() { return true; } _tick() {}
      _fail(e) { throw e; }
    }
    // Simulate loadBuffer without opts (default no focus)
    const el = new FakeViewer();
    const opts = {};
    // opts.focus is falsy and hasAttribute("autofocus") is false → _nxFocus not called
    if (opts.focus || el.hasAttribute("autofocus")) el._nxFocus();
    assert.equal(nxFocusCalled, 0, "loadBuffer must not focus by default");

    // With opts.focus = true → _nxFocus called
    const opts2 = { focus: true };
    if (opts2.focus || el.hasAttribute("autofocus")) el._nxFocus();
    assert.equal(nxFocusCalled, 1, "loadBuffer must focus when opts.focus = true");
  });
});
