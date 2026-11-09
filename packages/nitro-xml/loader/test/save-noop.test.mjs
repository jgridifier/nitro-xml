// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { WorkerDocument } from "../worker-host.js";
import { readFileSync } from "node:fs";
import { postSave } from "../lazy/save.js";
import { createJournal } from "../lazy/journal.js";

describe("noop save (worker dirty tracking)", () => {
  it("save() with no prior edit returns exact worker bytes unchanged", async () => {
    // Catches: WorkerDocument.save() wrapping or clipping bytes from the worker's
    // 'saved' message, turning a noop save into a corrupt package (#30/E1).
    const original = new Uint8Array([0x50, 0x4b, 3, 4, 1, 2, 3, 4, 5]);
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = null;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved") return { type: "saved", bytes: new Uint8Array(original) };
      return {};
    };
    const bytes = await doc.save();
    assert.ok(bytes instanceof Uint8Array, "save() must return Uint8Array");
    assert.deepEqual([...bytes], [...original]);
  });

  it("save() records to journal.markSave when journal exists (dirty path)", async () => {
    // Catches: journal not recording saved bytes, so edit-recovery cannot offer lastGoodSave.
    const original = new Uint8Array([0x50, 0x4b, 3, 4]);
    const journal = createJournal(original);
    journal.append({ spec: '{"command":"noop"}' });

    const savedBytes = new Uint8Array([0x50, 0x4b, 3, 4, 7, 8]);
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = journal;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved") return { type: "saved", bytes: new Uint8Array(savedBytes) };
      return {};
    };

    const result = await doc.save();
    assert.deepEqual([...result], [...savedBytes]);
    // originalBytes is unchanged — only markSave advances the lastGoodSave pointer.
    assert.deepEqual([...journal.originalBytes()], [...original]);
  });

  it("save() error from worker propagates as rejected promise (error surfacing)", async () => {
    // Catches: worker 'error' message during save being swallowed silently, so hosts
    // never learn save failed. Required by PRODUCT_SPEC §5 E1 (#30).
    const doc = Object.create(WorkerDocument.prototype);
    doc._journal = null;
    doc._pending = new Map();
    doc._req = 0;
    doc._request = async (expect) => {
      if (expect === "saved")
        throw Object.assign(new Error("disk full"), { code: "save_failed" });
      return {};
    };
    await assert.rejects(() => doc.save(), (err) => {
      assert.equal(err.message, "disk full");
      return true;
    });
  });

  it("worker saves edits from every command route and resets on reopen", async () => {
    // Catches: docx/pptx keyboard and mut-frame edits returning the unopened model
    // because only apply-edit marked the noop shortcut dirty. A corpus cannot
    // isolate worker message routing; selection-parity supplies real OOXML cases.
    const original = new Uint8Array([80, 75, 3, 4]);
    const edited = new Uint8Array([80, 75, 3, 4, 99]);
    const posted = [];
    class Handle {
      constructor(bytes) { this.bytes = bytes; }
      takePackageBytes() { return this.bytes; }
      saveDocument() { return edited; }
      openDocument() {}
      documentFormat() { return "docx"; }
      setReadOnly() {}
      loadEditorChrome() {}
      applyEdit() {}
      editorKey() {}
      shellAction() {}
      free() {}
    }
    const worker = { postMessage: (msg) => posted.push(msg) };
    // Execute production routing with only WASM/frame dependencies substituted.
    const source = readFileSync(new URL("../worker.js", import.meta.url), "utf8")
      .replace(/import[\s\S]*?from "[^"\n]+";/g, "")
      .replaceAll("await import(", "await loadModule(");
    const idle = new Function("self", "initCore", "isSupportedExtension",
      "applyFontSourceToHandle", "loadModule", source + "\nreturn () => chain;")(
      worker, async () => ({ glue: { Handle } }), () => true, () => {},
      async (path) => path.endsWith("save.js") ? { postSave } : {
        buildAndPostFrame: async () => {},
      },
    );
    async function send(msg) {
      worker.onmessage({ data: msg });
      await idle();
      assert.equal(posted.at(-1)?.type === "error", false, posted.at(-1)?.message);
    }
    async function saveEquals(expected) {
      await send({ type: "save" });
      assert.deepEqual(posted.at(-1).bytes, expected);
      assert.notEqual(posted.at(-1).bytes, expected);
    }
    for (const command of [
      { type: "apply-edit" },
      { type: "editor-key", key: "x" },
      { type: "sa", op: "insert-text", arg: "x" },
      { type: "mut-frame", op: "a" },
      { type: "mut-frame", op: "k", key: "x" },
      { type: "mut-frame", op: "s", sa: "insert-text", arg: "x" },
    ]) {
      await send({ type: "open", bytes: original, name: "test.docx", readonly: false });
      await send({ type: "editor-chrome" });
      await send({ type: "sa", op: "ui" });
      await send({ type: "sa", op: "zoom-set" });
      await saveEquals(original);
      await send(command);
      await saveEquals(edited);
      await send({ type: "original-bytes" });
      assert.deepEqual(posted.at(-1).bytes, original);
      await send({ type: "dispose" });
    }
  });
});
