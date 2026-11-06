import { test, expect, inspect } from './helpers/runtime.js';
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
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/iret_wrap/iret_wrap.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/iret-wrap-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'iret_wrap.hdm', mimeType: 'application/octet-stream', buffer: image });
  await inspect(page, () => pc98.core.set_boot_source(0));
  await page.locator('#reset').click();
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    await inspect(page, budget => {
      pc98.core.step(budget);
      pc98.core.audio_clear();
    }, manifest.slot_budget);
    expect(await inspect(page, () => pc98.stats().error)).toBe('');
    expect(await inspect(page, () => pc98.text())).toContain('IRET WRAP PASS');
    expect(await inspect(page, () => Array.from({ length: 3 }, (_, i) => pc98.read(0x8000 + i)))).toEqual([0xa5, 5, 0xa5]);
    expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
  }
});
