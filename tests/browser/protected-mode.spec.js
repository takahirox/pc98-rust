// Original MIT guest, production release Rust/Wasm only. Issue #101.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: paging-disabled 16/32-bit protected mode, faults and hardware IRQ, then real mode, cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/protected_mode/protected_mode.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/protected-mode-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await page.locator('#file').setInputFiles({ name: 'protected_mode.hdm', mimeType: 'application/octet-stream', buffer: image });
  await expect(page.locator('#media-name')).toContainText('protected_mode.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await page.evaluate(() => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('PROTECTED MODE PASS') || text.includes('PROTECTED MODE FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('PROTECTED MODE PASS');
    expect(text).not.toContain('FAIL');
    expect(await page.evaluate(() => Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(await page.evaluate(() => Array.from({ length: 6 }, (_, i) => pc98.read(0x210000 + i)))).toEqual([0xa5, 0xcd, 0xab, 0x34, 0x12, 0xa5]);
    expect(await page.evaluate(() => [pc98.read(0x18fe0), pc98.read(0x19000)])).toEqual([0xa5, 0xa5]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});
