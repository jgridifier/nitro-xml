// SPDX-License-Identifier: MIT
/**
 * applyEdit coalesce (microtask) + flush-on-tick.
 * Installed from worker-host-tick on first tick (lazy — off eager brotli budget).
 */

export function patch(WorkerDocument) {
  if (WorkerDocument._nxPark) return;
  WorkerDocument._nxPark = 1;

  const origEdit = WorkerDocument.prototype.applyEdit;
  const origTick = WorkerDocument.prototype.tick;

  async function flushEditQueue(doc) {
    if (doc._ef) return;
    doc._er = 0;
    const q = doc._eq;
    if (!q || !q.length) return;
    doc._eq = [];
    doc._ef = 1;
    try {
      for (const e of q) {
        try {
          e.resolve(await origEdit.call(doc, e.spec));
        } catch (err) {
          e.reject(err);
        }
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
        if (o && o.command === "setCellInput") key = `c:${o.sheet | 0}:${o.row | 0}:${o.col | 0}`;
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

  WorkerDocument.prototype.tick = async function (...args) {
    await flushEditQueue(this);
    return origTick.apply(this, args);
  };
}
