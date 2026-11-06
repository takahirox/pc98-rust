import { test, expect, inspect } from './helpers/runtime.js';
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
    expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const image = await fs.readFile(new URL('../../fixtures/pit_counter_latch/pit_counter_latch.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/pit-counter-latch-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'pit_counter_latch.hdm', mimeType: 'application/octet-stream', buffer: image });
    await inspect(page, () => pc98.core.set_boot_source(0));
    await page.locator('#reset').click();
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      let slots = 0;
      let text = '';
      // Guest programs and reads the PIT through both port banks.
      // Host only steps the production core and reads its outputs.
      while (slots < manifest.slot_budget) {
        const snapshot = await inspect(page, remaining => {
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
      const data = await inspect(page, () => Array.from({ length: 32 }, (_, i) => pc98.read(0x8000 + i)));
      expect(data[0]).toBe(expected[0]);
      expect(data.slice(9)).toEqual(expected.slice(9));
      for (const offset of [1,5]) {
        const held = data[offset] | data[offset+1] << 8;
        const fresh = data[offset+2] | data[offset+3] << 8;
        expect(held >> 8).toBe(0x12);
        expect(fresh).toBeGreaterThanOrEqual(0x0e00);
        expect(fresh).toBeLessThan(0x1100);
        expect(held-fresh).toBeGreaterThanOrEqual(0x100);
      }
      expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      console.log(`${name} boot ${run}: ${slots} slots`);
    }
  });
}
