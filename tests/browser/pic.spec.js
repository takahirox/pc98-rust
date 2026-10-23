import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: OCW2 priority and rotating EOI hardware IRQ trace before and after reset`, async ({ page }) => {
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const image = await fs.readFile(new URL('../../fixtures/pic/pic.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/pic-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await page.locator('#file').setInputFiles({ name: 'pic.hdm', mimeType: 'application/octet-stream', buffer: image });
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      await page.locator('#screen').focus();
      let slots = 0;
      let text = '';
      let nextPhase = 1;
      // Only guest execution writes the handshake, trace and result bytes.
      // Batch bounded stepping until an input handshake or terminal marker.
      while (slots < manifest.slot_budget) {
        const snapshot = await page.evaluate(({ nextPhase, remaining }) => {
          let slots = 0;
          while (slots < Math.min(100000, remaining)) {
            slots += pc98.core.step(1000);
            pc98.core.audio_clear();
            const phase = pc98.read(0x8001);
            const text = pc98.text();
            const error = pc98.stats().error;
            if (phase === nextPhase || text.includes('PIC PASS') || text.includes('FAIL') || error) {
              return { slots, phase, text, error };
            }
          }
          return { slots, phase: pc98.read(0x8001), text: pc98.text(), error: pc98.stats().error };
        }, { nextPhase, remaining: manifest.slot_budget - slots });
        slots += snapshot.slots;
        expect(snapshot.error).toBe('');
        text = snapshot.text;
        expect(text).not.toContain('FAIL');
        if (text.includes('PIC PASS')) break;
        if (snapshot.phase === nextPhase && nextPhase <= 3) {
          if (nextPhase === 2) await page.keyboard.up('Enter');
          else await page.keyboard.down('Enter');
          nextPhase++;
        }
      }
      expect(nextPhase).toBe(4);
      expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(text).toContain('PIC PASS');
      expect(await page.evaluate(() => Array.from({ length: 32 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      await page.keyboard.up('Enter');
    }
  });
}
