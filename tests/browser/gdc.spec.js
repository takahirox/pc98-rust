import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: original DOS guest observes horizontal blank on both GDCs in actual Wasm`, async ({ page }) => {
    // Only bounded explicit slices advance the guest, including during mounting.
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    await page.locator('#profile').selectOption(profile);
    const buffer = await fs.readFile(new URL('../../fixtures/freedos/hblank.hdm', import.meta.url));
    await page.locator('#file').setInputFiles({ name: 'hblank.hdm', mimeType: 'application/octet-stream', buffer });
    let text = '';
    for (let i = 0; i < 160; i++) {
      await page.evaluate(() => pc98.core.step(100000));
      expect((await page.evaluate(() => pc98.stats())).error).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('HBLANK PROBE RETURNED') && text.includes('A:\\>')) break;
    }
    expect(text).toContain('TEXT GDC 0060 PASS');
    expect(text).toContain('BITMAP GDC 00A0 PASS');
    expect(text).toContain('HBLANK TRANSITION PASS');
    expect(text).toContain('HBLANK PROBE RETURNED');
    expect(text).toContain('A:\\>');
    expect(text).not.toContain('TIMEOUT');
    await expect(page.locator('#error')).toBeHidden();
  });
}
