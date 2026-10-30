// Original MIT paged firmware guest; production Rust/Wasm at both roots.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: VM86 BIOS paging faults, mapped flags, callback and Enter, cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/vm86_firmware/vm86_firmware.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/vm86-firmware-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await page.locator('#file').setInputFiles({ name: 'probe.hdm', mimeType: 'application/octet-stream', buffer: image });
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    await page.locator('#screen').focus();
    let input = false;
    let text = '';
    for (let slots = 0; slots < manifest.slot_budget; slots += 10000) {
      const snapshot = await page.evaluate(() => {
        pc98.core.step(10000);
        pc98.core.audio_clear();
        return { ready: pc98.read(0x8000), text: pc98.text(), error: pc98.stats().error };
      });
      expect(snapshot.error).toBe('');
      text = snapshot.text;
      expect(text).not.toContain('FAIL');
      if (text.includes('VM86 BIOS PASS')) break;
      if (snapshot.ready === 1 && !input) {
        await page.keyboard.down('Enter');
        input = true;
      }
    }
    expect(input).toBe(true);
    expect(text).toContain('VM86 BIOS PASS');
    expect(await page.evaluate(() => Array.from({ length: 8 }, (_, i) => pc98.read(0x8000 + i)))).toEqual([2, 2, 0, 0, 1, 1, 13, 28]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await page.keyboard.up('Enter');
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
  }
});
