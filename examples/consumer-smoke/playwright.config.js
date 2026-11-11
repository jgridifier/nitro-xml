// SPDX-License-Identifier: MIT
import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: '.', testMatch: 'consumer.spec.js', workers: 1, timeout: 120000,
  use: { baseURL: 'http://127.0.0.1:5191', headless: true },
  webServer: { command: 'npm run preview', url: 'http://127.0.0.1:5191', reuseExistingServer: false },
});
