// SPDX-License-Identifier: MIT
// Catches: #516 inserting after the current slide leaves thumbnails on the original bytes.
// Run: node tests/interaction/pptx-thumbs.mjs (uses the existing built WASM package).
import assert from 'node:assert/strict';
import { chromium } from '../../parity/visual/node_modules/playwright/index.mjs';
import { startServer } from './server.mjs';

const server = await startServer();
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH });
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => { if (m.type() === 'error') { errors.push(m.text()); console.error(m.text()); } });
  await page.goto(`${server.url}/pptx-full.html`);
  await page.waitForFunction(() => document.querySelectorAll('.pl > button').length === 5);
  await page.locator('.pl .th[data-live]').first().waitFor();
  const text = (box) => box.shadowRoot?.querySelector('[aria-roledescription=slide]')?.textContent;
  async function check(expected) {
    await page.waitForFunction((n) => document.querySelectorAll('.pl > button').length === n, expected.length);
    for (let i = 0; i < expected.length; i++) {
      const button = page.locator('.pl > button').nth(i);
      await button.scrollIntoViewIfNeeded();
      await page.waitForFunction((i) => document.querySelectorAll('.pl .th')[i]?.dataset.live === '1', i).catch(async (e) => {
        console.error('missing thumbnail', i, errors, await page.evaluate(() => ({
          thumbs: [...document.querySelectorAll('.pl .th')].map(b => b.dataset.live),
          hosts: [...document.querySelectorAll('nitro-pptx')].map(el => ({ html: el._ch?.textContent, doc: !!el.document, dom: el.shadowRoot?.querySelector('.dom')?.textContent }))
        })));
        throw e;
      });
      assert.equal(await button.locator('.th').evaluate(text), expected[i], `thumbnail ${i + 1}`);
      await button.click();
      await page.waitForFunction(({ i, content }) => {
        const el = document.querySelector('nitro-pptx');
        return document.querySelectorAll('.pl > button')[i]?.getAttribute('aria-current') === 'true'
          && el.shadowRoot.querySelector('.dom > [aria-roledescription=slide]')?.textContent === content;
      }, { i, content: expected[i] });
    }
    assert.deepEqual(errors, []);
  }
  const original = [];
  for (let i = 0; i < 5; i++) {
    const box = page.locator('.pl .th').nth(i);
    await box.scrollIntoViewIfNeeded();
    await page.waitForFunction((i) => document.querySelectorAll('.pl .th')[i]?.dataset.live === '1', i);
    original.push(await box.evaluate(text));
  }
  await page.locator('.pl > button').first().click();
  await page.getByRole('button', { name: 'New Slide', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('[data-pos]')?.textContent === 'Slide 2 of 6');
  assert.equal(await page.locator('.pl > button[aria-current=true]').getAttribute('data-g'), '1');
  await check([original[0], '', ...original.slice(1)]);
  // Duplicate a nonblank middle slide, then delete it, including keyboard undo/redo.
  await page.locator('.pl > button').nth(2).click();
  await page.getByRole('button', { name: 'Duplicate Slide', exact: true }).click();
  const duplicated = [original[0], '', original[1], ...original.slice(1)];
  await check(duplicated);
  await page.locator('.pl > button').nth(3).click();
  await page.getByRole('button', { name: 'Delete Slide', exact: true }).click();
  await check([original[0], '', ...original.slice(1)]);
  for (const [key, expected] of [['Control+z', duplicated], ['Control+y', [original[0], '', ...original.slice(1)]]]) {
    await page.locator('nitro-pptx').first().evaluate((el) => el._v.focus());
    await page.keyboard.press(key);
    await check(expected);
  }
  // Replace the hidden host again while its new thumbnails are still queued.
  await page.locator('.pl > button').first().click();
  for (const count of [7, 8]) {
    await page.getByRole('button', { name: 'New Slide', exact: true }).click();
    await page.waitForFunction((n) => document.querySelectorAll('.pl > button').length === n, count);
  }
  await check([original[0], '', '', '', ...original.slice(1)]);
  console.log('pptx-thumbs: PASS (add, duplicate, delete, keyboard undo/redo; every thumbnail matches canvas; no page errors)');
} finally {
  await browser.close();
  server.close();
}
