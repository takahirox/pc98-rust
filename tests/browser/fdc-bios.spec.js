import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: BIOS takes idle ND=1 and permits PIO reentry before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/fdc_bios/fdc_bios.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/fdc-bios-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const write0 = Buffer.from(Array.from({ length: 1024 }, (_, i) => (29 * i + 71) & 255));
    const write2 = Buffer.from(Array.from({ length: 1024 }, (_, i) => (13 * i + 0xb3) & 255));
    const expectedMedia = Buffer.from(image);
    write0.copy(expectedMedia, 2048);
    write2.copy(expectedMedia, 34816);
    expect(createHash('sha256').update(expectedMedia).digest('hex')).toBe(manifest.expected_media_sha256);
    const expectedRam = Buffer.alloc(0x4410, 0xa5);
    expectedRam[0] = 1;
    expectedRam.fill(0, 1, 15); // seven AH/CF pairs from actual INT 1Bh
    for (const [address, bytes] of [
      [0x9000, image.subarray(1024, 2048)], [0x9800, image.subarray(33792, 34816)],
      [0xa000, write0], [0xa800, write2], [0xb000, write2],
      [0xb800, image.subarray(33792, 34816)], [0xc000, image.subarray(1024, 2048)],
    ]) bytes.copy(expectedRam, address - 0x8000);
    await page.locator('#file').setInputFiles({ name: 'fdc_bios.hdm', mimeType: 'application/octet-stream', buffer: image });
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      // Guest issues every command, verifies results and produces PASS itself.
      const result = await page.evaluate(budget => {
        let slots = 0;
        let text = '';
        while (slots < budget) {
          slots += pc98.core.step(Math.min(1000, budget - slots));
          pc98.core.audio_clear();
          text = pc98.text();
          if (text.includes('FDC BIOS PASS') || text.includes('FAIL') || pc98.stats().error) break;
        }
        return { slots, text, error: pc98.stats().error,
          ram: Array.from({ length: 0x4410 }, (_, i) => pc98.read(0x8000 + i)),
          writes: pc98.stats().writes };
      }, manifest.slot_budget);
      expect(result.slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(result.error).toBe('');
      expect(result.text).not.toContain('FAIL');
      expect(result.text).toContain('FDC BIOS PASS');
      expect(result.ram.slice(0, 32)).toEqual([...Buffer.from(manifest.record_hex, 'hex')]);
      expect(Buffer.from(result.ram).equals(expectedRam)).toBe(true);
      expect(result.writes).toBe(2);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(expectedMedia)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      console.log(`${name} FDC BIOS boot ${run}: ${result.slots} slots, seven INT 1Bh calls, two PIO reads and two writes`);
    }
  });
}
