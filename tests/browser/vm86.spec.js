// Original MIT guest, production release Rust/Wasm only. Issue #109.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: VM86, CPL3 VM86 PG=0/1, TSS monitor gates, bitmap/IOPL, #PF and PIC/PIT IRQ, cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/vm86/vm86.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/vm86-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await page.locator('#file').setInputFiles({ name: 'vm86.hdm', mimeType: 'application/octet-stream', buffer: image });
  await expect(page.locator('#media-name')).toContainText('vm86.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await page.evaluate(() => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('VM86 PASS') || text.includes('VM86 FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('VM86 PASS');
    expect(text).not.toContain('FAIL');
    expect(await page.evaluate(() => Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    for (const address of [0x17f80, 0x18000, 0x66ff0, 0x67000, 0x116ff0, 0x117000]) {
      expect(await page.evaluate(a => Array.from({ length: 4 }, (_, i) => pc98.read(a + i)), address)).toEqual([0xa5, 0xa5, 0xa5, 0xa5]);
    }
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});
