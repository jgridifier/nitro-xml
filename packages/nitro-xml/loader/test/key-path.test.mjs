// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createEventCoalescer, EventKind } from "../events.js";

const HERE = dirname(fileURLToPath(import.meta.url));

describe("key-path hygiene (theme 8)", () => {
  it("flushes coalesced pointer events before a later key would see them", () => {
    // Catches: editorKey racing ahead of rAF-batched pointer events so
    // click-then-immediate-ArrowDown acts on the old cell (S3-1).
    const prevRaf = globalThis.requestAnimationFrame;
    const prevCancel = globalThis.cancelAnimationFrame;
    globalThis.requestAnimationFrame = () => 1;
    globalThis.cancelAnimationFrame = () => {};
    try {
      const batches = [];
      const c = createEventCoalescer((bytes) => {
        batches.push(bytes);
      });
      c.push({
        kind: EventKind.Pointer,
        time_ms: 1,
        x: 10,
        y: 10,
        type: 0,
        button: 0,
        buttons: 1,
        shiftKey: false,
        ctrlKey: false,
        altKey: false,
        metaKey: false,
      });
      assert.equal(c.length, 1);
      c.flushNow();
      assert.equal(c.length, 0);
      assert.equal(batches.length, 1);
    } finally {
      globalThis.requestAnimationFrame = prevRaf;
      globalThis.cancelAnimationFrame = prevCancel;
    }
  });

  it("reserves formula-bar height while chrome is empty", () => {
    // Catches: first chrome mount shifting the grid ~29px under the pointer (S2-9).
    const src = readFileSync(join(HERE, "../xlsx-element.js"), "utf8");
    assert.match(src, /\.chrome:empty\{min-height:29px/);
    const host = readFileSync(join(HERE, "../worker-host.js"), "utf8");
    assert.match(host, /flushNow\(\);\s*\nconst msg = await this\._request\("editor-key"/);
  });
});
