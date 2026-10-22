#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// Catches: @nitro-xml/react evaluating window/document at module load (breaks SSR).
import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const DIST = join(HERE, "../dist/index.js");

test("NitroViewer module is importable without DOM globals", async () => {
  // Catches: top-level customElements.define / window access in @nitro-xml/react.
  const g = globalThis;
  const hadWindow = "window" in g;
  const hadDocument = "document" in g;
  const savedWindow = g.window;
  const savedDocument = g.document;
  try {
    // Simulate a minimal SSR/Node environment (no browser globals).
    // eslint-disable-next-line no-delete-var
    try {
      delete g.window;
    } catch {
      g.window = undefined;
    }
    try {
      delete g.document;
    } catch {
      g.document = undefined;
    }
    const mod = await import(pathToFileURL(DIST).href + `?t=${Date.now()}`);
    assert.equal(typeof mod.NitroViewer, "object");
    assert.equal(mod.NitroViewer.displayName, "NitroViewer");
  } finally {
    if (hadWindow) g.window = savedWindow;
    else delete g.window;
    if (hadDocument) g.document = savedDocument;
    else delete g.document;
  }
});

test("package peerDependencies pin nitro-xml without bundling it", () => {
  // Catches: shipping wasm inside @nitro-xml/react (must stay a peer of nitro-xml).
  const require = createRequire(import.meta.url);
  const pkg = require("../package.json");
  assert.ok(pkg.peerDependencies["nitro-xml"]);
  assert.equal(pkg.sideEffects, false);
});
