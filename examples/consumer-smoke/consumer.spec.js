// SPDX-License-Identifier: MIT
// Catches: packed exports, workers or WASM failing in a built consumer, or save dropping edits.
import { test, expect } from '@playwright/test';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const cases = JSON.parse(readFileSync('cases.json'));
for (const entry of cases) {
  test(entry.id, async ({ page }) => {
    // Catches: packaged worker boot failures and edits lost by the real React save path.
    const failures = [];
    page.on('pageerror', e => failures.push(e.message));
    await page.goto(`/?file=${encodeURIComponent(entry.file)}`);
    await expect(page.locator('#status')).not.toHaveText('loading', { timeout: 90000 });
    await expect(page.locator('#status')).toHaveText('ready');
    await expect(page.locator(`nitro-${entry.format}`)).toBeVisible();
    const saved = await page.evaluate(async entry => {
      for (const { op, ...args } of entry.ops)
        await window.consumer.applyEdit({ command: op, ...args });
      return Array.from(await window.consumer.save());
    }, entry);
    expect(saved.length).toBeGreaterThan(1000);
    expect(saved.slice(0, 2)).toEqual([80, 75]);
    expect(failures).toEqual([]);
    const original = readFileSync(`public/fixtures/${entry.file}`);
    expect(Buffer.from(saved).equals(original), 'edit must change saved bytes').toBe(false);
    const out = process.env.NITRO_CONSUMER_OUT;
    mkdirSync(out, { recursive: true });
    writeFileSync(join(out, `${entry.id}.${entry.format}`), Buffer.from(saved));
  });
}
