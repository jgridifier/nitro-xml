// SPDX-License-Identifier: MIT
function copyBytes(bytes) {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return new Uint8Array(u8);
}
function cloneOp(op) {
  return typeof structuredClone === "function" ? structuredClone(op) : JSON.parse(JSON.stringify(op));
}
/**
 * Append-only edit op journal for crash recovery (ADR 0003 / #62).
 * Ops use the same shape as edits/*.json entries.
 *
 * @param {Uint8Array|ArrayBuffer|number[]} originalBytes
 * @param {{ ops?: object[], lastGoodSave?: Uint8Array|ArrayBuffer|number[] }} [opts]
 */
export function createJournal(originalBytes, opts = {}) {
  const original = copyBytes(originalBytes);
  const ops = Array.isArray(opts.ops) ? opts.ops.map(cloneOp) : [];
  let lastGoodSave =
    opts.lastGoodSave != null && opts.lastGoodSave !== undefined
      ? copyBytes(opts.lastGoodSave)
      : null;
  return {
    originalBytes() {
      return copyBytes(original);
    },
    append(op) {
      ops.push(cloneOp(op));
    },
    ops() {
      return ops.map(cloneOp);
    },
    snapshot() {
      return {
        originalBytes: copyBytes(original),
        ops: ops.map(cloneOp),
        lastGoodSave: lastGoodSave ? copyBytes(lastGoodSave) : null,
      };
    },
    markSave(blob) {
      lastGoodSave = copyBytes(blob);
    },
    lastGoodSave() {
      return lastGoodSave ? copyBytes(lastGoodSave) : null;
    },
    /**
     * Open originals in a fresh session and replay committed journal ops.
     *
     * @param {{
     *   openFresh: (original: Uint8Array) => Promise<unknown>|unknown,
     *   applyOps: (session: unknown, op: object) => Promise<void>|void,
     *   fingerprint?: (session: unknown) => Promise<string>|string,
     *   expectedFingerprint?: string,
     * }} ctx
     */
    async recover(ctx) {
      const last = lastGoodSave ? copyBytes(lastGoodSave) : null;
      let session;
      try {
        session = await ctx.openFresh(copyBytes(original));
      } catch (err) {
        return { ok: false, lastGoodSave: last, error: err };
      }
      try {
        for (const op of ops) {
          await ctx.applyOps(session, op);
        }
      } catch (err) {
        return { ok: false, lastGoodSave: last, error: err };
      }
      if (ctx.fingerprint != null && ctx.expectedFingerprint != null) {
        const fp = await ctx.fingerprint(session);
        if (fp !== ctx.expectedFingerprint) {
          return {
            ok: false,
            lastGoodSave: last,
            error: new Error("journal replay fingerprint mismatch"),
          };
        }
      }
      return { ok: true, session, lastGoodSave: last };
    },
  };
}
