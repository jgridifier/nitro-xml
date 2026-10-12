// SPDX-License-Identifier: MIT
/**
 * Viewer reload-once harness (#30): recoverViewer with crash injection.
 *
 * Usage: bun tests/crash/viewer_reload.mjs [--ci]
 */

import {
  recoverViewer,
  VIEWER_RECOVERY_FAILED_MESSAGE,
} from "../../packages/nitro-xml/loader/test/viewer-recover-lib.mjs";

const seed = Number(process.env.NITRO_CRASH_SEED || "30");

async function runHappyPath() {
  const originalBytes = new Uint8Array([0x50, 0x4b, 0x03, 0x04, seed & 0xff]);
  // Viewing worker crashed; reload-once opens a fresh worker with the frozen bytes.
  let reloadOpens = 0;
  const res = await recoverViewer({
    originalBytes,
    name: "tiny.xlsx",
    opts: { readonly: true },
    attempted: false,
    openFresh: async (bytes, name, opts) => {
      reloadOpens++;
      assertBytes(bytes, originalBytes);
      return { byteLength: bytes.length, name, readonly: opts.readonly };
    },
  });
  if (!res.ok) {
    throw new Error(`viewer reload happy path failed: ${res.error}`);
  }
  if (reloadOpens !== 1) {
    throw new Error(`expected exactly one reload open, got ${reloadOpens}`);
  }
  console.log(`crash-recovery: viewer reload happy path OK (seed=${seed})`);
}

async function runNegativeControl() {
  let opens = 0;
  const openFresh = async () => {
    opens++;
    throw new Error("injected reload crash");
  };
  const first = await recoverViewer({
    originalBytes: new Uint8Array([9]),
    attempted: false,
    openFresh,
  });
  if (first.ok) {
    console.error("crash-recovery: viewer NEGATIVE CONTROL BROKEN (crash reported ok)");
    process.exit(1);
  }
  if (!first.attempted) {
    console.error("crash-recovery: viewer NEGATIVE CONTROL BROKEN (attempted not set)");
    process.exit(1);
  }
  const second = await recoverViewer({
    originalBytes: new Uint8Array([9]),
    attempted: true,
    openFresh,
  });
  if (second.ok) {
    console.error("crash-recovery: viewer NEGATIVE CONTROL BROKEN (third open allowed)");
    process.exit(1);
  }
  if (second.error.message !== VIEWER_RECOVERY_FAILED_MESSAGE) {
    console.error("crash-recovery: viewer NEGATIVE CONTROL BROKEN (wrong exhausted message)");
    process.exit(1);
  }
  if (opens !== 1) {
    console.error(`crash-recovery: viewer NEGATIVE CONTROL BROKEN (opens=${opens}, expected 1)`);
    process.exit(1);
  }
  console.log("crash-recovery: viewer reload negative control OK");
}

function assertBytes(a, b) {
  if (a.length !== b.length) throw new Error("bytes length mismatch");
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) throw new Error("bytes mismatch");
  }
}

async function main() {
  await runHappyPath();
  await runNegativeControl();
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
