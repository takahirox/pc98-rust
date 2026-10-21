import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

const pattern = Array.from({ length: 32 }, (_, i) => i % 2 === 0
  ? 0x80 >> (Math.floor(i / 2) % 8) : (i * 7) ^ 0x53);
async function until(page, predicate) {
  for (let i = 0; i < 180; i++) {
    if (await predicate()) return;
    await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
    expect(await page.evaluate(() => pc98.stats().error)).toBe('');
  }
  throw Error('font guest did not reach expected state');
}
async function boot(page, name) {
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  const buffer = await fs.readFile(new URL(`../../fixtures/freedos/${name}.hdm`, import.meta.url));
  await page.locator('#file').setInputFiles({ name: `${name}.hdm`, mimeType: 'application/octet-stream', buffer });
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
}
async function assertGlyph(page, expected) {
  const result = await page.evaluate(() => {
    const p = pc98.core.frame();
    const pixels = new Uint8Array(pc98.core.memory.buffer, p, 640 * 400 * 4);
    return {
      bios: Array.from({ length: 32 }, (_, i) => pc98.read(0x90004 + i)),
      cg: Array.from({ length: 32 }, (_, i) => pc98.read(0x90040 + i)),
      glyphs: [0, 32].map(left => Array.from({ length: 16 }, (_, y) =>
        Array.from({ length: 16 }, (_, x) => {
          const i = ((160 + y) * 640 + left + x) * 4;
          return Array.from(pixels.subarray(i, i + 4));
        }))),
    };
  });
  expect(result.bios).toEqual(expected);
  expect(result.cg).toEqual(expected);
  for (let g = 0; g < 2; g++) for (let y = 0; y < 16; y++) for (let x = 0; x < 16; x++) {
    const ink = Boolean(expected[y * 2 + (x >> 3)] & (128 >> (x & 7)));
    expect(result.glyphs[g][y][x], `glyph ${g} pixel ${x},${y}`).toEqual(
      ink ? (g === 0 ? [255, 255, 255, 255] : [0, 0, 255, 255]) : [0, 0, 0, 255]);
  }
}

test('Ap BIOS user font: original FreeDOS control, readback, text/graphics pixels, replace and reset', async ({ page }) => {
  test.setTimeout(60000);
  // Only bounded explicit Wasm steps advance the probe, so both stages and
  // reset can be inspected before a later guest instruction changes them.
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await boot(page, 'fontload');
  await until(page, () => page.evaluate(() => pc98.text().includes('FONTLOAD RETURNED')));
  await boot(page, 'userfont');
  await until(page, () => page.evaluate(() => pc98.read(0x90000) === 1));
  await assertGlyph(page, pattern);
  await page.locator('#screen').focus();
  await page.keyboard.press('Enter');
  await until(page, () => page.evaluate(() => pc98.read(0x90000) === 2));
  await assertGlyph(page, pattern.map(v => v ^ 255));
  await page.keyboard.press('Enter');
  await until(page, () => page.evaluate(() => pc98.text().includes('USERFONT RETURNED')));
  expect(await page.evaluate(() => pc98.text())).not.toContain('USERFONT FAILED');
  await page.evaluate(() => pc98.core.reset());
  expect(await page.evaluate(() => pc98.read(0x90000))).toBe(0);
  // Reset drops the font RAM; reboot must load it again and restore first pixels.
  await until(page, () => page.evaluate(() => pc98.read(0x90000) === 1));
  await assertGlyph(page, pattern);
  await expect(page.locator('#error')).toBeHidden();
});
