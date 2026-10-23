import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: SMSW readback, exact widths and guest fault repair before and after reset', async ({ page }) => {
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/smsw/smsw.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/smsw-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await page.locator('#file').setInputFiles({ name: 'smsw.hdm', mimeType: 'application/octet-stream', buffer: image });
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await page.evaluate(() => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('SMSW PASS') || text.includes('SMSW FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('SMSW PASS');
    expect(text).not.toContain('FAIL');
    expect(await page.evaluate(() => Array.from({ length: 80 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(await page.evaluate(() => Array.from({ length: 3 }, (_, i) => pc98.read(0xfffd + i)))).toEqual([...Buffer.from(manifest.last_operand_hex, 'hex')]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
  }
});
