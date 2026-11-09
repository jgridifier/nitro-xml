#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Catches: NitroViewer swallowing initialization errors silently — hosts can't
// react to open/load failures (silent failure identified in E1 / #30).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const TYPES_FILE = join(HERE, "../dist/NitroViewer.d.ts");
const DIST_FILE = join(HERE, "../dist/index.js");

test("NitroViewerProps declares onError for host error surfacing", () => {
  // Catches: NitroViewer dropping initialization errors silently (#30/E1).
  // onError must be in the type so hosts can react to open/load failures.
  const dts = readFileSync(TYPES_FILE, "utf8");
  assert.ok(
    dts.includes("onError?: (err: Error) => void"),
    "NitroViewerProps must include onError?: (err: Error) => void",
  );
});

test("compiled NitroViewer wires onError into the effect catch handler", () => {
  // Catches: onError declared in types but never called in the compiled output.
  const js = readFileSync(join(HERE, "../dist/NitroViewer.js"), "utf8");
  assert.ok(js.includes("onErrorRef"), "onErrorRef must appear in compiled JS");
  assert.ok(
    js.includes("onErrorRef.current"),
    "onErrorRef.current must be called in compiled JS",
  );
});

test("module is importable without DOM globals (SSR regression)", async () => {
  // Catches: top-level customElements.define / window access in @nitro-xml/react.
  const g = globalThis;
  const hadWindow = "window" in g;
  const hadDocument = "document" in g;
  const savedWindow = g.window;
  const savedDocument = g.document;
  try {
    try { delete g.window; } catch { g.window = undefined; }
    try { delete g.document; } catch { g.document = undefined; }
    const mod = await import(pathToFileURL(DIST_FILE).href + `?t=${Date.now()}`);
    assert.equal(typeof mod.NitroViewer, "object");
    assert.equal(mod.NitroViewer.displayName, "NitroViewer");
  } finally {
    if (hadWindow) g.window = savedWindow; else delete g.window;
    if (hadDocument) g.document = savedDocument; else delete g.document;
  }
});
