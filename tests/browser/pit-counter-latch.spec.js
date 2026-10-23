import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: PIT counter-latch duplicate retention and recapture before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/pit_counter_latch/pit_counter_latch.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/pit-counter-latch-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await page.locator('#file').setInputFiles({ name: 'pit_counter_latch.hdm', mimeType: 'application/octet-stream', buffer: image });
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      let slots = 0;
      let text = '';
      // Guest programs and reads the PIT through both port banks.
      // Host only steps the production core and reads its outputs.
      while (slots < manifest.slot_budget) {
        const snapshot = await page.evaluate(remaining => {
          let slots = 0;
          let text = '';
          while (slots < Math.min(100000, remaining)) {
            slots += pc98.core.step(Math.min(1000, remaining - slots));
            text = pc98.text();
            const error = pc98.stats().error;
            if (text.includes('PIT COUNTER LATCH PASS') || text.includes('FAIL') || error) {
              return { slots, text, error };
            }
          }
          return { slots, text, error: pc98.stats().error };
        }, manifest.slot_budget - slots);
        slots += snapshot.slots;
        expect(snapshot.error).toBe('');
        text = snapshot.text;
        expect(text).not.toContain('FAIL');
        if (text.includes('PIT COUNTER LATCH PASS')) break;
      }
      expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(text).toContain('PIT COUNTER LATCH PASS');
      expect(await page.evaluate(() => Array.from({ length: 32 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      console.log(`${name} boot ${run}: ${slots} slots`);
    }
  });
}
