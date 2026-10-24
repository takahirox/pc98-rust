import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: single-mode PIT/FDC and restored cascade IRQ before and after reset`, async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('console', message => {
      if (message.type() === 'error') errors.push(message.text());
    });
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const image = await fs.readFile(new URL('../../fixtures/pic_single/pic_single.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/pic-single-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await page.locator('#file').setInputFiles({ name: 'pic_single.hdm', mimeType: 'application/octet-stream', buffer: image });
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      // The host only steps and reads: all requests and results originate in guest code.
      const result = await page.evaluate(budget => {
        let slots = 0;
        let text = '';
        while (slots < budget) {
          slots += pc98.core.step(Math.min(1000, budget - slots));
          pc98.core.audio_clear();
          text = pc98.text();
          if (text.includes('PIC SINGLE PASS') || text.includes('FAIL') || pc98.stats().error) break;
        }
        return { slots, text, error: pc98.stats().error,
          bytes: Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)) };
      }, manifest.slot_budget);
      expect(result.slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(result.error).toBe('');
      expect(result.text).not.toContain('FAIL');
      expect(result.text).toContain('PIC SINGLE PASS');
      expect(result.bytes).toEqual(expected);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
    }
  });
}
