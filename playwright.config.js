import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/browser',
  timeout: 45_000,
  expect: { timeout: 12_000 },
  fullyParallel: true,
  use: { browserName: 'chromium', headless: true, trace: 'retain-on-failure' },
  projects: [
    { name: 'local', use: { baseURL: 'http://127.0.0.1:8000/' } },
    { name: 'pages', use: { baseURL: 'http://127.0.0.1:8001/pc98-rust/' } },
  ],
  webServer: [
    { command: 'npm run serve', url: 'http://127.0.0.1:8000', reuseExistingServer: false, timeout: 15_000 },
    { command: 'python3 tests/browser/serve_project.py', url: 'http://127.0.0.1:8001/pc98-rust/', reuseExistingServer: false, timeout: 15_000 },
  ],
});
