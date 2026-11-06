import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: MOV CR reads to all GPRs, prefixes and guest fault repair before and after reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/mov_cr/mov_cr.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/mov-cr-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'mov_cr.hdm', mimeType: 'application/octet-stream', buffer: image });
  await inspect(page, () => pc98.core.set_boot_source(0));
  await page.locator('#reset').click();
  await expect(page.locator('#media-name')).toContainText('mov_cr.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await inspect(page, () => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await inspect(page, () => pc98.stats().error)).toBe('');
      text = await inspect(page, () => pc98.text());
      if (text.includes('MOV CR PASS') || text.includes('MOV CR FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('MOV CR PASS');
    expect(text).not.toContain('FAIL');
    expect(await inspect(page, () => Array.from({ length: 128 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});
