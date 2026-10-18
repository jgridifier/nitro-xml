// SPDX-License-Identifier: MIT
/**
 * Catches: over MAX_UNCOMPRESSED_BYTES / zip-bomb packages crashing or hanging the viewer
 * instead of a clean size-limit error (#30 memory ceiling).
 *
 * Builds a tiny stored zip whose central-directory declared sizes sum past the 300 MiB
 * package ceiling (no large binary checked in). Opens via the wasm Handle for xlsx, docx
 * and pptx name hints, and asserts the loader maps the code to PACKAGE_TOO_LARGE_MESSAGE.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  PACKAGE_TOO_LARGE_MESSAGE,
  PACKAGE_LIMIT_CODES,
  viewerOpenErrorMessage,
} from "../formats.js";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const PKG_DIR = join(ROOT, "packages/nitro-xml/pkg");
const PKG = join(PKG_DIR, "nitro_xml_wasm.js");
const WASM = join(PKG_DIR, "nitro_xml_wasm_bg.wasm");

/** CRC-32 (IEEE) for stored zip local headers. */
function crc32(bytes) {
  let c = ~0;
  for (let i = 0; i < bytes.length; i++) {
    c ^= bytes[i];
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return ~c >>> 0;
}

/**
 * Stored (method 0) zip. Each part is `{ name, data, declared }` — headers may lie about
 * uncompressed size so we can trip MAX_UNCOMPRESSED_BYTES without writing 300 MiB.
 * Built with concatenated Uint8Arrays (no spread) so ~164 KiB payloads do not blow the stack.
 */
function storedZip(parts) {
  const chunks = [];
  const centralChunks = [];
  let outLen = 0;
  let centralLen = 0;
  const enc = new TextEncoder();
  for (const { name, data, declared } of parts) {
    const nameBytes = enc.encode(name);
    const offset = outLen;
    const crc = crc32(data);
    const comp = data.length;
    const local = new Uint8Array(30 + nameBytes.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 20, true);
    lv.setUint16(8, 0, true); // stored
    lv.setUint32(14, crc, true);
    lv.setUint32(18, comp, true);
    lv.setUint32(22, declared >>> 0, true);
    lv.setUint16(26, nameBytes.length, true);
    local.set(nameBytes, 30);
    chunks.push(local, data);
    outLen += local.length + data.length;

    const cd = new Uint8Array(46 + nameBytes.length);
    const cv = new DataView(cd.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 20, true);
    cv.setUint16(6, 20, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, comp, true);
    cv.setUint32(24, declared >>> 0, true);
    cv.setUint16(28, nameBytes.length, true);
    cv.setUint32(42, offset >>> 0, true);
    cd.set(nameBytes, 46);
    centralChunks.push(cd);
    centralLen += cd.length;
  }
  const eocd = new Uint8Array(22);
  const ev = new DataView(eocd.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, parts.length, true);
  ev.setUint16(10, parts.length, true);
  ev.setUint32(12, centralLen, true);
  ev.setUint32(16, outLen, true);

  const out = new Uint8Array(outLen + centralLen + eocd.length);
  let o = 0;
  for (const c of chunks) {
    out.set(c, o);
    o += c.length;
  }
  for (const c of centralChunks) {
    out.set(c, o);
    o += c.length;
  }
  out.set(eocd, o);
  return out;
}

/**
 * 160 MiB + 160 MiB declared → 320 MiB > 300 MiB MAX_UNCOMPRESSED_BYTES.
 * Payload is sized so declared/compressed stays under MAX_COMPRESSION_RATIO (1000), so the
 * default open path hits the aggregate ceiling — not the per-part ratio guard.
 */
function overTotalUncompressedZip() {
  const per = 160 * 1024 * 1024;
  // compressed * 1000 >= declared  ⇒  compressed >= ceil(declared / 1000)
  const minComp = Math.ceil(per / 1000);
  const a = new Uint8Array(minComp);
  a[0] = 1;
  const b = new Uint8Array(minComp);
  b[0] = 2;
  return storedZip([
    { name: "xl/a.bin", data: a, declared: per },
    { name: "xl/b.bin", data: b, declared: per },
  ]);
}

function ensureWasm() {
  if (existsSync(PKG) && existsSync(WASM)) return;
  const r = spawnSync("bash", ["scripts/build-wasm.sh"], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  if (r.status !== 0) throw new Error(`build-wasm: ${r.stderr || r.stdout}`);
}

describe("package size limits (#30)", () => {
  it("viewerOpenErrorMessage maps limit codes to PACKAGE_TOO_LARGE_MESSAGE", () => {
    // Catches: loader showing a raw ZipError string (or empty) for size-limit codes.
    for (const code of PACKAGE_LIMIT_CODES) {
      assert.equal(
        viewerOpenErrorMessage({ code, message: "internal detail" }),
        PACKAGE_TOO_LARGE_MESSAGE,
      );
    }
    assert.notEqual(
      viewerOpenErrorMessage({ code: "open_failed", message: "other" }),
      PACKAGE_TOO_LARGE_MESSAGE,
    );
  });

  it("openDocument rejects an over-MAX_UNCOMPRESSED zip for xlsx, docx and pptx without hanging", async () => {
    // Catches: wasm open hanging/panicking on a declared-total zip bomb instead of
    // returning total_too_large / compression_ratio for every format hint.
    ensureWasm();
    const { initSync, Handle } = await import(PKG);
    initSync({ module: readFileSync(WASM) });
    const bytes = overTotalUncompressedZip();
    // ~328 KiB generated payload — not checked in; far under any fixture budget.
    assert.ok(bytes.byteLength < 512 * 1024, "fixture must stay small (generated, not checked in)");

    for (const name of ["bomb.xlsx", "bomb.docx", "bomb.pptx"]) {
      const h = new Handle(bytes);
      let err;
      try {
        h.openDocument(name);
      } catch (e) {
        err = e;
      }
      h.free();
      assert.ok(err, `${name}: expected open to throw`);
      assert.equal(
        err.code,
        "total_too_large",
        `${name}: expected total_too_large, got ${err.code} (${err.message})`,
      );
      assert.equal(viewerOpenErrorMessage(err), PACKAGE_TOO_LARGE_MESSAGE);
    }
  });
});
