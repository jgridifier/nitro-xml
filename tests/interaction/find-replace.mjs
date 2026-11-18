// SPDX-License-Identifier: MIT
// Catches: Word replace-all failing to preserve styled body/table text, save, or one-step undo.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { mkdirSync } from 'node:fs';
import { zipEntries } from './lib.mjs';
import { startServer } from './server.mjs';
const require = createRequire(new URL('../../parity/visual/package.json', import.meta.url));
const { chromium } = require('playwright');
const server = await startServer();
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH });
try {
 const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
 await page.goto(`${process.env.DEMO_URL || server.url}/docx-full.html`);
 page.on('pageerror', e => console.error('pageerror', e.message));
 const el = page.locator('nitro-docx');
 await el.locator('.dve-print-page').waitFor();
 const save = async () => Buffer.from(await el.evaluate(async e => Array.from(await e.save())));
 const xml = bytes => zipEntries(bytes)['word/document.xml']().toString();
 const before = xml(await save());
 await el.locator('.dom').click({ position: { x: 300, y: 200 } });
 await page.keyboard.press('Control+h');
 await el.getByRole('textbox', { name: 'Replace with', exact: true }).fill('Q4');
 await el.getByRole('searchbox', { name: 'Find', exact: true }).fill('Q3');
 await el.getByRole('button', { name: 'Replace all', exact: true }).click();
 await el.getByText('Replaced 2', { exact: true }).waitFor({ timeout: 10000 }).catch(async e => { console.error(await el.locator('.f').innerText()); throw e; });
 await page.locator('.ox[data-dirty]').waitFor();
 const changed = await save();
 assert.equal(xml(changed).includes('Q3'), false);
 assert.equal((xml(changed).match(/Q4/g) || []).length, 2);
 const shots = process.env.REPLACE_SHOTS || fileURLToPath(new URL('./out', import.meta.url));
 mkdirSync(shots, { recursive: true });
 await page.screenshot({ path: `${shots}/feature-542-replace-docx.png` });
 await page.keyboard.press('Control+z');
 await page.waitForFunction(() => document.querySelector('nitro-docx').shadowRoot.querySelector('.dom').textContent.includes('Q3'));
 assert.equal(xml(await save()), before);
 await page.waitForFunction(() => !document.querySelector('.ox')?.hasAttribute('data-dirty'));
 // Reopen through the demo's public element API, then inspect both rendered text and saved XML.
 await page.locator('input[type=file]').setInputFiles({ name: 'replaced.docx', mimeType: 'application/octet-stream', buffer: changed });
 await page.waitForFunction(() => document.querySelector('nitro-docx').shadowRoot.querySelector('.dom').textContent.includes('Q4'));
 await el.locator('.dve-print-page').waitFor();
 assert.equal(xml(await save()).includes('Q3'), false);
 await el.locator('.dom').click({ position: { x: 300, y: 200 } });
 await page.keyboard.press('Control+h');
 await el.getByRole('searchbox', { name: 'Find', exact: true }).fill('Q4');
 await el.locator('.fs').filter({ hasText: 'of 2' }).waitFor();
 await el.getByRole('textbox', { name: 'Replace with', exact: true }).fill('');
 await el.getByRole('button', { name: 'Find next', exact: true }).click();
 await el.locator('.fs').filter({ hasText: '2 of 2' }).waitFor();
 await el.locator('.nx-replace').getByRole('button', { name: 'Replace', exact: true }).click();
 await el.getByText('Replaced 1', { exact: true }).waitFor();
 const single = xml(await save());
 assert.equal((single.match(/Q4/g) || []).length, 1);
 assert.match(single, /Q4 summary/);
 console.log('PASS Word replace-all, single Ctrl+Z, save/reopen, selected table match and empty replacement');
} finally { await browser.close(); server.close(); }
