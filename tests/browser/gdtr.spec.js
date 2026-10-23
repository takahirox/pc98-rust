import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

test('Ap: original GDTR boot probe checks exact bytes and faults before and after reset in release Wasm', async ({ page }) => {
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/gdtr/gdtr.hdm', import.meta.url));
  await page.locator('#file').setInputFiles({ name: 'gdtr.hdm', mimeType: 'application/octet-stream', buffer: image });
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let text = '';
    for (let i = 0; i < 20; i++) {
      await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('GDTR PASS') || text.includes('GDTR FAIL')) break;
    }
    expect(text).toContain('GDTR PASS');
    expect(text).not.toContain('FAIL');
    const full = [0x34, 0x12, 0x78, 0x56, 0x34, 0x12];
    const expected = Array(80).fill(0xa5);
    for (const offset of [1, 9, 25, 33, 41, 57, 65]) expected.splice(offset, 6, ...full);
    expected.splice(17, 6, 0x34, 0x12, 0x78, 0x56, 0x34, 0);
    expected.fill(0, 49, 55);
    expected[73] = 4;
    expect(await page.evaluate(() => Array.from({ length: 80 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(await page.evaluate(() => Array.from({ length: 7 }, (_, i) => pc98.read(0xfff9 + i)))).toEqual([0xa5, ...full]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
  }
});
