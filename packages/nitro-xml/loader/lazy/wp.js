// SPDX-License-Identifier: MIT
/**
 * rAF applyEdit coalesce. Worker park: takePrewarmedWorker(w) on dispose.
 * Installed from worker-host-tick on first tick (lazy — off eager brotli budget).
 */

export function patch(WorkerDocument) {
  if (WorkerDocument._nxPark) return;
  WorkerDocument._nxPark = 1;

  const origEdit = WorkerDocument.prototype.applyEdit;
  WorkerDocument.prototype.applyEdit = function (spec) {
    const s = spec || "";
    if (typeof requestAnimationFrame !== "function") return origEdit.call(this, s);
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
      this._er = requestAnimationFrame(async () => {
        this._er = 0;
        const q = this._eq || [];
        this._eq = [];
        for (const e of q) {
          try {
            e.resolve(await origEdit.call(this, e.spec));
          } catch (err) {
            e.reject(err);
          }
        }
      });
    });
  };
}
