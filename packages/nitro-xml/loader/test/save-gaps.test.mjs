// SPDX-License-Identifier: MIT
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { PACKAGE_LIMIT_CODES } from "../formats.js";

// Catches: WASM hanging on the >25 MiB declared-part fixture for the full 120 s worker
// deadline instead of returning a size-limit error immediately (#503 / #30).
// The 120 s budget is the fallback for truly stuck WASM; size limits must fire under 2 s.

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const PKG_DIR = join(ROOT, "packages/nitro-xml/pkg");
const PKG = join(PKG_DIR, "nitro_xml_wasm.js");
const WASM = join(PKG_DIR, "nitro_xml_wasm_bg.wasm");
const FIXTURE = join(ROOT, "fixtures/fuzz/zip/docx-document-over-25mib.zip");

describe("save-gate gaps (#30/E1)", () => {
  it(
    "oversized fixture rejected with too-large code in < 2000 ms (WASM required; skip locally)",
    async (t) => {
      // Catches: a zip whose largest part is declared >25 MiB causing WASM to process for
      // 120 s (the existing worker-deadline budget) rather than returning a size-limit error.
      // The two rejection paths are distinct: this test asserts the size path, not the timeout.
      if (!existsSync(PKG) || !existsSync(WASM)) {
        t.skip();
        return;
      }
      const { initSync, Handle } = await import(PKG);
      initSync({ module: readFileSync(WASM) });

      const bytes = readFileSync(FIXTURE);
      assert.ok(bytes.byteLength > 0, "fixture must be non-empty");

      const before = performance.now();
      let err;
      const h = new Handle(bytes);
      try {
        h.openDocument("test.docx");
      } catch (e) {
        err = e;
      } finally {
        h.free();
      }
      const elapsed = performance.now() - before;

      assert.ok(err, "oversized fixture must throw a size-limit error");
      assert.ok(
        PACKAGE_LIMIT_CODES.includes(err.code),
        `expected one of ${PACKAGE_LIMIT_CODES.join("|")} but got code=${err.code} message=${err.message}`,
      );
      assert.ok(
        elapsed < 2000,
        `too-large rejection must arrive in < 2000 ms (got ${elapsed.toFixed(1)} ms); ` +
          `a 120 s hang would indicate the size check is missing and the worker-deadline budget fires instead`,
      );
    },
  );
});
