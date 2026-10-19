// SPDX-License-Identifier: MIT
/** Test-only mirror of viewer one-shot recovery orchestration (#30). */
import { VIEWER_RECOVERY_FAILED_MESSAGE } from "../formats.js";

export { VIEWER_RECOVERY_FAILED_MESSAGE };

export async function recoverViewer(c) {
  if (c.attempted) {
    return {
      ok: false,
      attempted: true,
      error: Object.assign(new Error(VIEWER_RECOVERY_FAILED_MESSAGE), {
        code: "viewer_recovery_exhausted",
      }),
    };
  }
  const u8 =
    c.originalBytes instanceof Uint8Array
      ? new Uint8Array(c.originalBytes)
      : new Uint8Array(c.originalBytes);
  try {
    return {
      ok: true,
      attempted: true,
      session: await c.openFresh(u8, c.name || "", c.opts || {}),
    };
  } catch (e) {
    const r =
      e instanceof Error
        ? e
        : Object.assign(new Error(String(e)), { cause: e });
    if (!r.code) r.code = "viewer_recovery_failed";
    return { ok: false, attempted: true, error: r };
  }
}
