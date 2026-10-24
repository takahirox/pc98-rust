import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: non-DMA multi-sector FDC write/readback and data/result IRQs before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/fdc_write_multi/fdc_write_multi.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/fdc-write-multi-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    const payload = Buffer.from(Array.from({ length: 2048 }, (_, i) => (29 * i + 71 + Math.floor(i / 1024) * 91) & 255));
    const expectedMedia = Buffer.from(image);
    payload.copy(expectedMedia, 1024);
    expect(createHash('sha256').update(expectedMedia).digest('hex')).toBe(manifest.expected_media_sha256);
    await page.locator('#file').setInputFiles({ name: 'fdc_write_multi.hdm', mimeType: 'application/octet-stream', buffer: image });
    // File.arrayBuffer() and the mount/reset finish after the change event.
    await expect(page.locator('#media-name')).toHaveText('fdc_write_multi.hdm');
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
          if (text.includes('FDC WRITE MULTI PASS') || text.includes('FAIL') || pc98.stats().error) break;
        }
        return { slots, text, error: pc98.stats().error,
          bytes: Array.from({ length: 128 }, (_, i) => pc98.read(0x8000 + i)),
          source: Array.from({ length: 2048 }, (_, i) => pc98.read(0x9000 + i)),
          payload: Array.from({ length: 2048 }, (_, i) => pc98.read(0x9800 + i)),
          guards: Array.from({ length: 0xf80 }, (_, i) => pc98.read(0x8080 + i))
            .concat(Array.from({ length: 16 }, (_, i) => pc98.read(0xa000 + i))),
          writes: pc98.stats().writes };
      }, manifest.slot_budget);
      expect(result.slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(result.error).toBe('');
      expect(result.text).not.toContain('FAIL');
      expect(result.text).toContain('FDC WRITE MULTI PASS');
      expect(result.bytes).toEqual(expected);
      expect(Buffer.from(result.source).equals(payload)).toBe(true);
      expect(Buffer.from(result.payload).equals(payload)).toBe(true);
      expect(createHash('sha256').update(Buffer.from(result.payload)).digest('hex')).toBe(manifest.payload_sha256);
      expect(result.guards).toEqual(Array(0xf90).fill(0xa5));
      expect(result.writes).toBe(2);
      console.log(`${name} FDC WRITE MULTI boot ${run}: ${result.slots} slots, 2048 write + 2048 read byte IRQs and 2 result IRQs`);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(expectedMedia)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
    }
  });
}
