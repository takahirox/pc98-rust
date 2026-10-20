import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/browser',
  timeout: 45_000,
  expect: { timeout: 12_000 },
  fullyParallel: true,
  use: { baseURL: 'http://127.0.0.1:8000', browserName: 'chromium', headless: true, trace: 'retain-on-failure' },
  webServer: { command: 'npm run serve', url: 'http://127.0.0.1:8000', reuseExistingServer: false, timeout: 15_000 },
});
