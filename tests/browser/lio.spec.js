import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: ROM-registered GINIT returns through real DOS in production Wasm`, async ({ page }) => {
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const buffer = await fs.readFile(new URL('../../fixtures/freedos/lio.hdm', import.meta.url));
    await page.locator('#file').setInputFiles({ name: 'lio.hdm', mimeType: 'application/octet-stream', buffer });
    // Observe the entire bounded run, so a later reboot cannot pass unnoticed.
    for (let i = 0; i < 160; i++) {
      await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
      expect((await page.evaluate(() => pc98.stats())).error).toBe('');
    }
    const text = await page.evaluate(() => pc98.text());
    for (const message of ['LIO ENTER', 'LIO RETURNED', 'PROBE SHELL RETURN', 'GINIT CONTRACT PASS', 'A:\\>']) {
      expect(text).toContain(message);
    }
    expect(text).not.toContain('FAILED');
    expect(text.match(/LIO ENTER/g)).toHaveLength(1);
    const pixels = await page.evaluate(() => {
      const pointer = pc98.core.frame();
      const data = new Uint8Array(pc98.core.memory.buffer, pointer, 640 * 400 * 4);
      return [300, 301, 302, 303].map(y => {
        const p = (y * 640 + 632) * 4;
        return Array.from(data.subarray(p, p + 4));
      });
    });
    expect(pixels).toEqual([[0, 0, 255, 255], [0, 0, 255, 255],
      [255, 0, 0, 255], [255, 0, 0, 255]]);
    await expect(page.locator('#error')).toBeHidden();
    // Reset rebuilds the directory and the same DOS controls work a second time.
    await page.evaluate(() => pc98.core.reset());
    expect(await page.evaluate(() => pc98.read(0xf9900))).toBe(17);
    for (let i = 0; i < 160; i++) {
      await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
    }
    expect((await page.evaluate(() => pc98.stats())).error).toBe('');
    expect(await page.evaluate(() => pc98.text())).toContain('GINIT CONTRACT PASS');
    expect(await page.evaluate(() => pc98.text())).not.toContain('FAILED');
  });
}
