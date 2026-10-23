import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: OCW3 special-mask hardware IRQ nesting before and after reset`, async ({ page }) => {
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
    const image = await fs.readFile(new URL('../../fixtures/pic_smm/pic_smm.hdm', import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/pic-smm-manifest.json', import.meta.url)));
    expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
    const expected = [...Buffer.from(manifest.result_hex, 'hex')];
    await page.locator('#file').setInputFiles({ name: 'pic_smm.hdm', mimeType: 'application/octet-stream', buffer: image });
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      await page.locator('#screen').focus();
      let slots = 0;
      let text = '';
      let sentKey = false;
      // Host supplies physical input only; the guest owns every result byte.
      while (slots < manifest.slot_budget) {
        const snapshot = await page.evaluate(({ sentKey, remaining }) => {
          let slots = 0;
          while (slots < Math.min(100000, remaining)) {
            slots += pc98.core.step(1000);
            pc98.core.audio_clear();
            const phase = pc98.read(0x8001);
            const text = pc98.text();
            const error = pc98.stats().error;
            if ((!sentKey && phase === 1) || text.includes('PIC SMM PASS') || text.includes('FAIL') || error) {
              return { slots, phase, text, error };
            }
          }
          return { slots, phase: pc98.read(0x8001), text: pc98.text(), error: pc98.stats().error };
        }, { sentKey, remaining: manifest.slot_budget - slots });
        slots += snapshot.slots;
        expect(snapshot.error).toBe('');
        text = snapshot.text;
        expect(text).not.toContain('FAIL');
        if (text.includes('PIC SMM PASS')) break;
        if (snapshot.phase === 1 && !sentKey) {
          await page.keyboard.down('Enter');
          sentKey = true;
        }
      }
      expect(sentKey).toBe(true);
      expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
      expect(text).toContain('PIC SMM PASS');
      expect(await page.evaluate(() => Array.from({ length: 32 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
      expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      await expect(page.locator('#error')).toBeHidden();
      await page.keyboard.up('Enter');
    }
  });
}
