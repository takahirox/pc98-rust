// Original MIT guest, production release Rust/Wasm only. Issue #105.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: CLTS, real/protected TS/EM #NM recovery and WAIT, cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/clts/clts.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/clts-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await page.locator('#file').setInputFiles({ name: 'clts.hdm', mimeType: 'application/octet-stream', buffer: image });
  await expect(page.locator('#media-name')).toContainText('clts.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await page.evaluate(() => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('CLTS PASS') || text.includes('CLTS FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('CLTS PASS');
    expect(text).not.toContain('FAIL');
    expect(await page.evaluate(() => Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    for (const address of [0x7b00, 0x7c00]) {
      expect(await page.evaluate(a => [pc98.read(a), pc98.read(a + 1)], address)).toEqual([0xa5, 0xa5]);
    }
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});
