// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import fs from "node:fs";
import { describe, it } from "node:test";

describe("scoreboard first-paint harness", () => {
  it("avoids host initCore so cold open does not double-instantiate wasm", () => {
    // Catches: paintNitroXlsx/Pptx calling initCore() on the page before
    // WorkerDocument open, doubling wasm instantiate vs React-embed preload().
    const src = fs.readFileSync(
      new URL(
        "../../../../perf/scoreboard/races/render/page/harness.mjs",
        import.meta.url,
      ),
      "utf8",
    );
    const code = src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
    assert.doesNotMatch(code, /\binitCore\s*\(/);
    assert.doesNotMatch(code, /\binitCore\b/);
    assert.match(code, /setDefaultFontSource/);
    assert.match(code, /preload/);
  });

});
