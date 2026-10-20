// SPDX-License-Identifier: MIT
/**
 * applyEdit coalesce (microtask) + mut-frame edit+paint (one worker RT).
 * Installed from worker-host-tick on first tick (lazy — off eager brotli budget).
 *
 * mut-frame is docx/pptx only: on xlsx it forced a full frame build per edit and
 * regressed cell/bold latency and heap (see #341). Xlsx keeps coalesce + separate tick.
 */

import { extensionOf } from "../formats.js";

function isSpreadsheet(doc) {
  const ext = extensionOf(doc && doc._name);
  return ext === "xlsx" || ext === "xlsm" || ext === "xltx" || ext === "xltm";
}

export function patch(WorkerDocument) {
  if (WorkerDocument._nxPark) return;
  WorkerDocument._nxPark = 1;

  const origEdit = WorkerDocument.prototype.applyEdit;
  const origTick = WorkerDocument.prototype.tick;
  const origKey = WorkerDocument.prototype.editorKey;
  const origSa = WorkerDocument.prototype.shellAction;

  async function mutFrame(doc, fields) {
    const msg = await doc._request("mut-frame", () => {
      doc._worker.postMessage({ type: "mut-frame", ...fields });
    });
    doc._xf = msg;
    return msg.result;
  }

  async function flushEditQueue(doc) {
    if (doc._ef) return;
    doc._er = 0;
    const q = doc._eq;
    if (!q || !q.length) return;
    doc._eq = [];
    doc._ef = 1;
    try {
      const last = q[q.length - 1];
      for (let i = 0; i < q.length - 1; i++) {
        const e = q[i];
        try {
          e.resolve(await origEdit.call(doc, e.spec));
        } catch (err) {
          e.reject(err);
        }
      }
      try {
        // Spreadsheet: coalesce only — frame stays on the next tick (no xlsx regress).
        if (
          !isSpreadsheet(doc) &&
          doc._worker &&
          typeof doc._request === "function"
        ) {
          const result = await mutFrame(doc, { op: "a", spec: last.spec });
          const frame = doc._xf;
          doc._xf = null;
          if (frame && frame.commands) {
            const { paintFrame } = await import("./paint-frame.js");
            await paintFrame(doc, frame);
            doc._nxPainted = 1;
          }
          last.resolve(result);
        } else {
          last.resolve(await origEdit.call(doc, last.spec));
        }
      } catch (err) {
        last.reject(err);
      }
    } finally {
      doc._ef = 0;
    }
  }

  WorkerDocument.prototype.applyEdit = function (spec) {
    const s = spec || "";
    return new Promise((resolve, reject) => {
      if (!this._eq) this._eq = [];
      let key = null;
      try {
        const o = JSON.parse(s);
        if (o && o.command === "setCellInput")
          key = `c:${o.sheet | 0}:${o.row | 0}:${o.col | 0}`;
      } catch {
        /* raw */
      }
      const item = { spec: s, key, resolve, reject };
      if (key) {
        const i = this._eq.findIndex((e) => e.key === key);
        if (i >= 0) {
          const prev = this._eq[i];
          item.resolve = (r) => {
            prev.resolve(r);
            resolve(r);
          };
          item.reject = (e) => {
            prev.reject(e);
            reject(e);
          };
          this._eq[i] = item;
        } else this._eq.push(item);
      } else this._eq.push(item);
      if (this._er) return;
      this._er = 1;
      const run = () => {
        if (!this._er) return;
        void flushEditQueue(this);
      };
      if (typeof queueMicrotask === "function") queueMicrotask(run);
      else Promise.resolve().then(run);
    });
  };

  async function mutFrameAndPaint(doc, fields) {
    const result = await mutFrame(doc, fields);
    const frame = doc._xf;
    doc._xf = null;
    if (frame && frame.commands) {
      const { paintFrame } = await import("./paint-frame.js");
      await paintFrame(doc, frame);
      doc._nxPainted = 1;
    }
    return result;
  }

  WorkerDocument.prototype.editorKey = async function (ev, mac) {
    if (isSpreadsheet(this) || !(this._worker && typeof this._request === "function")) {
      return origKey.call(this, ev, mac);
    }
    const result = await mutFrameAndPaint(this, {
      op: "k",
      key: (ev && ev.key) || "",
      ctrl: !!(ev && ev.ctrlKey),
      meta: !!(ev && ev.metaKey),
      alt: !!(ev && ev.altKey),
      shift: !!(ev && ev.shiftKey),
      mac: !!mac,
    });
    try {
      return JSON.parse(result || "{}");
    } catch {
      return {};
    }
  };

  WorkerDocument.prototype.shellAction = async function (op, arg = "") {
    // Open-path ui/zoom must stay cheap (no forced frame).
    if (op === "ui" || op === "zoom-set") return origSa.call(this, op, arg);
    if (
      isSpreadsheet(this) ||
      !(this._worker && typeof this._request === "function")
    ) {
      return origSa.call(this, op, arg);
    }
    const result = await mutFrameAndPaint(this, {
      op: "s",
      sa: op || "ui",
      arg: arg || "",
    });
    try {
      return JSON.parse(result);
    } catch {
      return { ui: null };
    }
  };

  WorkerDocument.prototype.tick = async function (...args) {
    await flushEditQueue(this);
    const frame = this._xf;
    this._xf = null;
    if (frame && frame.commands) {
      const { paintFrame } = await import("./paint-frame.js");
      return paintFrame(this, frame);
    }
    if (this._nxPainted) {
      this._nxPainted = 0;
      return { ops: [], measured: 0, outlined: 0, layoutReads: 0 };
    }
    return origTick.apply(this, args);
  };
}
