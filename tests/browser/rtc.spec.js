import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['1', 'Ap']]) {
  test(`${name}: RTC serial calendar and coherent BIOS reads before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/rtc/rtc.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/rtc-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    await page.locator('#file').setInputFiles({ name: 'rtc.hdm', mimeType: 'application/octet-stream', buffer: image });
    await expect(page.locator('#media-name')).toHaveText('rtc.hdm');
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      const expected = [...Buffer.from(run ? manifest.reset_result_hex : manifest.result_hex, 'hex')];
      let slots = 0;
      let text = '';
      // Guest checks real serial I/O, BIOS agreement, advancing seconds and guards.
      // Host only steps the production core and reads its outputs.
      while (slots < manifest.slot_budget) {
        const snapshot = await page.evaluate(remaining => {
          let slots = 0;
          let text = '';
          while (slots < Math.min(100000, remaining)) {
            slots += pc98.core.step(Math.min(1000, remaining - slots));
            text = pc98.text();
            const error = pc98.stats().error;
            if (text.includes('RTC CALENDAR PASS') || text.includes('FAIL') || error) {
              return { slots, text, error };
            }
          }
          return { slots, text, error: pc98.stats().error };
        }, manifest.slot_budget - slots);
        slots += snapshot.slots;
        expect(snapshot.error).toBe('');
        text = snapshot.text;
        expect(text).not.toContain('FAIL');
        if (text.includes('RTC CALENDAR PASS')) break;
      }
      expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(text).toContain('RTC CALENDAR PASS');
      expect(await page.evaluate(() => Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      console.log(`${name} boot ${run}: ${slots} slots`);
    }
  });
}
