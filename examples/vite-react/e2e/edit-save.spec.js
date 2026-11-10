// SPDX-License-Identifier: MIT
/**
 * Catches: Vite + @nitro-xml/react failing to open a format, apply one edit, or
 * return non-empty save bytes (regression of the npm embed path Jared ships).
 * Also validates: noop save returns original bytes; onError surfaces load failures.
 *
 * Saved bytes are written to NITRO_REACT_E2E_OUT (default: /tmp/nitro-react-e2e-out)
 * so the CI script can pass them to the Open XML SDK validator and LibreOffice.
 */
import { test, expect } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const SAVE_OUT =
  process.env.NITRO_REACT_E2E_OUT ?? join(tmpdir(), "nitro-react-e2e-out");

function writeSaved(name, bytes) {
  mkdirSync(SAVE_OUT, { recursive: true });
  writeFileSync(join(SAVE_OUT, name), Buffer.from(bytes));
}

const FORMATS = ["xlsx", "docx", "pptx"];

for (const format of FORMATS) {
  test(`opens ${format}, edits once, and saves valid OOXML bytes`, async ({ page }) => {
    await page.goto("/");
    if (format !== "xlsx") await page.getByTestId(`format-${format}`).click();
    await expect(page.getByTestId("status")).toContainText("ready=yes", {
      timeout: 90_000,
    });

    await page.getByTestId("edit-save").click();
    await expect(page.getByTestId("status")).toContainText(/saved:\d+/, {
      timeout: 60_000,
    });

    const n = await page.evaluate(() => {
      const b = window.__nitroLastSave;
      return b ? b.byteLength : 0;
    });
    expect(n).toBeGreaterThan(1000);

    // ZIP local-file header signature for OOXML packages
    const sig = await page.evaluate(() => {
      const b = window.__nitroLastSave;
      return b && b[0] === 0x50 && b[1] === 0x4b;
    });
    expect(sig).toBe(true);

    // Write to disk for CI validator (ooxml-validate + LibreOffice).
    const savedArray = await page.evaluate(() =>
      Array.from(window.__nitroLastSave ?? []),
    );
    if (savedArray.length > 0) {
      writeSaved(`saved_edit.${format}`, savedArray);
    }
  });
}

test("noop save (xlsx, no edit) returns byte-identical to input fixture", async ({ page }) => {
  // Catches: save() repacking the ZIP unnecessarily when no edit applied,
  // producing bytes that differ from the original input (#30/E1).
  await page.goto("/");
  await expect(page.getByTestId("status")).toContainText("ready=yes", {
    timeout: 90_000,
  });

  await page.getByTestId("noop-save").click();
  await expect(page.getByTestId("status")).toContainText(/noop-saved:\d+/, {
    timeout: 60_000,
  });

  // Compare saved bytes to original fixture bytes fetched from the dev server.
  const result = await page.evaluate(async () => {
    const saved = window.__nitroNoopSave;
    if (!saved) return { match: false, reason: "no saved bytes" };
    const res = await fetch("/fixtures/cells_1k.xlsx");
    if (!res.ok) return { match: false, reason: `fetch failed: ${res.status}` };
    const original = new Uint8Array(await res.arrayBuffer());
    if (saved.byteLength !== original.byteLength)
      return {
        match: false,
        reason: `length mismatch: saved=${saved.byteLength} original=${original.byteLength}`,
      };
    for (let i = 0; i < saved.byteLength; i++) {
      if (saved[i] !== original[i])
        return { match: false, reason: `byte differs at offset ${i}` };
    }
    return { match: true };
  });
  expect(result.match, `noop save mismatch: ${result.reason ?? ""}`).toBe(true);

  // Also write noop-saved bytes for validator.
  const noopArray = await page.evaluate(() =>
    Array.from(window.__nitroNoopSave ?? []),
  );
  if (noopArray.length > 0) {
    writeSaved("saved_noop.xlsx", noopArray);
  }
});

for (const { format, fixtureUrl } of [
  { format: "docx", fixtureUrl: "/fixtures/pages_10.docx" },
  { format: "pptx", fixtureUrl: "/fixtures/slides_10.pptx" },
]) {
  test(`noop save (${format}, no edit) returns byte-identical to input fixture`, async ({ page }) => {
    // Catches: takePackageBytes() for docx/pptx returning re-packed bytes instead of the
    // original ZIP when no edit has been applied (#30/E1 — xlsx was already covered).
    await page.goto("/");
    await page.getByTestId(`format-${format}`).click();
    await expect(page.getByTestId("status")).toContainText("ready=yes", {
      timeout: 90_000,
    });

    await page.getByTestId("noop-save").click();
    await expect(page.getByTestId("status")).toContainText(/noop-saved:\d+/, {
      timeout: 60_000,
    });

    const result = await page.evaluate(async (url) => {
      const saved = window.__nitroNoopSave;
      if (!saved) return { match: false, reason: "no saved bytes" };
      const res = await fetch(url);
      if (!res.ok) return { match: false, reason: `fetch failed: ${res.status}` };
      const original = new Uint8Array(await res.arrayBuffer());
      if (saved.byteLength !== original.byteLength)
        return {
          match: false,
          reason: `length mismatch: saved=${saved.byteLength} original=${original.byteLength}`,
        };
      for (let i = 0; i < saved.byteLength; i++) {
        if (saved[i] !== original[i])
          return { match: false, reason: `byte differs at offset ${i}` };
      }
      return { match: true };
    }, fixtureUrl);
    expect(result.match, `noop save mismatch: ${result.reason ?? ""}`).toBe(true);

    const noopArray = await page.evaluate(() =>
      Array.from(window.__nitroNoopSave ?? []),
    );
    if (noopArray.length > 0) {
      writeSaved(`saved_noop.${format}`, noopArray);
    }
  });
}

test("onError is called when NitroViewer fails to load (error surfacing)", async ({ page }) => {
  // Catches: NitroViewer swallowing initialization errors silently — hosts
  // cannot display or log load failures (#30/E1).
  await page.route("**/fixtures/cells_1k.xlsx", (route) => route.abort("failed"));
  await page.goto("/");

  // The error should surface via onError → setStatus → data-testid="status".
  await expect(page.getByTestId("status")).toContainText("error:", {
    timeout: 15_000,
  });
});
