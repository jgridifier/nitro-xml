// SPDX-License-Identifier: MIT
/**
 * Catches: Vite + @nitro-xml/react failing to open a format, apply one edit, or
 * return non-empty save bytes (regression of the npm embed path Jared ships).
 */
import { test, expect } from "@playwright/test";

const FORMATS = ["xlsx", "docx", "pptx"];

for (const format of FORMATS) {
  test(`opens ${format}, edits once, and saves bytes`, async ({ page }) => {
    await page.goto("/");
    await page.getByTestId(`format-${format}`).click();
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
  });
}
