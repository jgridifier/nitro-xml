// SPDX-License-Identifier: MIT
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { CSS as EMBEDDED } from "../lazy/embedded/style.js";
import { CSS as RIBBON } from "../lazy/ribbon/style.js";
import { THEMES, applyTheme, resolveTheme } from "../lazy/theme.js";

const fake = () => {
  const vars = new Map();
  const attrs = new Map();
  return {
    vars,
    attrs,
    style: { setProperty: (k, v) => vars.set(k, v), removeProperty: (k) => vars.delete(k) },
    setAttribute: (k, v) => attrs.set(k, v),
    removeAttribute: (k) => attrs.delete(k),
  };
};

describe("chrome theme tokens", () => {
  it("sets data-theme and --nx-* on the host, and a switch drops the previous theme's variables", () => {
    // Catches: switching quiet -> custom leaving the old accent/font behind on the host.
    const el = fake();
    applyTheme(el, "quiet");
    assert.equal(el.attrs.get("data-theme"), "quiet");
    assert.equal(el.vars.get("--nx-accent"), THEMES.quiet.accent);
    assert.ok(el.vars.get("--nx-ink") && el.vars.get("--nx-bg-subtle") && el.vars.get("--nx-font-sans"));
    applyTheme(el, { "--nx-accent": "#0a7", fontSans: "Georgia" });
    assert.equal(el.attrs.get("data-theme"), "custom");
    assert.equal(el.vars.get("--nx-accent"), "#0a7");
    assert.equal(el.vars.get("--nx-font-sans"), "Georgia");
    assert.equal(el.vars.has("--nx-ink"), false);
    applyTheme(el, null);
    assert.equal(el.vars.size, 0);
    assert.equal(el.attrs.has("data-theme"), false);
  });

  it("office leaves the per-app accent to the frame; quiet and dark set their own", () => {
    // Catches: office pinning one accent so Excel green / PowerPoint orange collapse to the same colour.
    assert.equal("accent" in THEMES.office, false);
    assert.ok(THEMES.quiet.accent && THEMES.dark.accent);
  });

  it("rejects unknown names and declaration-injection values", () => {
    // Catches: a host-supplied token value such as `red;} body{display:none` escaping its declaration.
    assert.throws(() => resolveTheme("nope"), /unknown theme/);
    assert.deepEqual(resolveTheme({ accent: "red;} body{display:none", ink: "#111" }).vars, { ink: "#111" });
    assert.equal(resolveTheme('{"accent":"#123"}').vars.accent, "#123");
  });

  it("ribbon and embedded styles read every core token (no hard-coded chrome palette)", () => {
    // Catches: a restyle that re-hardcodes chrome colours so hosts can no longer theme without forking.
    for (const t of ["ink", "ink-muted", "bg", "bg-subtle", "border", "accent", "accent-contrast", "sel", "font-sans", "radius"])
      assert.ok(RIBBON.includes(`--nx-${t}`), `ribbon CSS does not read --nx-${t}`);
    for (const t of ["ink", "bg-subtle", "border", "accent", "font-sans", "radius"])
      assert.ok(EMBEDDED.includes(`--nx-${t}`), `embedded CSS does not read --nx-${t}`);
  });
});
