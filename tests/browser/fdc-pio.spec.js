import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: non-DMA FDC payload and data/result IRQs before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/fdc_pio/fdc_pio.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/fdc-pio-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await page.locator('#file').setInputFiles({ name: 'fdc_pio.hdm', mimeType: 'application/octet-stream', buffer: image });
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
          if (text.includes('FDC PIO PASS') || text.includes('FAIL') || pc98.stats().error) break;
        }
        return { slots, text, error: pc98.stats().error,
          bytes: Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)),
          payload: Array.from({ length: 2048 }, (_, i) => pc98.read(0x9000 + i)),
          guards: Array.from({ length: 0xfc0 }, (_, i) => pc98.read(0x8040 + i))
            .concat(Array.from({ length: 16 }, (_, i) => pc98.read(0x9800 + i))),
          writes: pc98.stats().writes };
      }, manifest.slot_budget);
      expect(result.slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(result.error).toBe('');
      expect(result.text).not.toContain('FAIL');
      expect(result.text).toContain('FDC PIO PASS');
      expect(result.bytes).toEqual(expected);
      expect(Buffer.from(result.payload).equals(image.subarray(1024, 3072))).toBe(true);
      expect(createHash('sha256').update(Buffer.from(result.payload)).digest('hex')).toBe(manifest.payload_sha256);
      expect(result.guards).toEqual(Array(0xfd0).fill(0xa5));
      expect(result.writes).toBe(0);
      console.log(`${name} FDC PIO boot ${run}: ${result.slots} slots, 2048 data IRQs and 1 result IRQ`);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
    }
  });
}
