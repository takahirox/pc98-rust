import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: original DMA auto-init boot probe reuses raw-port read/write buffers in production Wasm`, async ({ page }) => {
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const image = await fs.readFile(new URL('../../fixtures/dma/autoinit.hdm', import.meta.url));
    await page.locator('#file').setInputFiles({ name: 'autoinit.hdm', mimeType: 'application/octet-stream', buffer: image });
    let text = '';
    for (let i = 0; i < 20; i++) {
      await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
      expect(await page.evaluate(() => pc98.stats().error)).toBe('');
      text = await page.evaluate(() => pc98.text());
      if (text.includes('DMA AUTO INIT PASS') || text.includes('DMA AUTO INIT FAIL')) break;
    }
    expect(text).toContain('DMA AUTO INIT PASS');
    expect(text).not.toContain('FAIL');
    const expected = Buffer.from(image);
    expected.fill(0x69, 4 * 1024, 5 * 1024);
    expected.fill(0xb6, 5 * 1024, 6 * 1024);
    const actual = Buffer.from(await page.evaluate(() => pc98.exportDisk()));
    expect(actual.equals(expected)).toBe(true);
    const buffer = await page.evaluate(() => Array.from({ length: 1024 }, (_, i) => pc98.read(0x8000 + i)));
    expect(buffer.every(byte => byte === 0xb6)).toBe(true);
    expect(await page.evaluate(() => [pc98.read(0x7fff), pc98.read(0x8400)])).toEqual([0xa5, 0xa5]);
    await expect(page.locator('#error')).toBeHidden();
  });
}
