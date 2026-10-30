import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: real IRET wraps between operands, BIOS return and flags survive cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/iret_wrap/iret_wrap.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/iret-wrap-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await page.locator('#file').setInputFiles({ name: 'iret_wrap.hdm', mimeType: 'application/octet-stream', buffer: image });
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    await page.evaluate(budget => {
      pc98.core.step(budget);
      pc98.core.audio_clear();
    }, manifest.slot_budget);
    expect(await page.evaluate(() => pc98.stats().error)).toBe('');
    expect(await page.evaluate(() => pc98.text())).toContain('IRET WRAP PASS');
    expect(await page.evaluate(() => Array.from({ length: 3 }, (_, i) => pc98.read(0x8000 + i)))).toEqual([0xa5, 5, 0xa5]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
  }
});
