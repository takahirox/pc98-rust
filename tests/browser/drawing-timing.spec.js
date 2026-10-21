import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

async function boot(page, name) {
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  const buffer = await fs.readFile(new URL(`../../fixtures/freedos/${name}.hdm`, import.meta.url));
  await page.locator('#file').setInputFiles({ name: `${name}.hdm`, mimeType: 'application/octet-stream', buffer });
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
}
async function until(page, predicate) {
  for (let i = 0; i < 160; i++) {
    if (await predicate()) return;
    await page.evaluate(() => { pc98.core.step(100000); pc98.core.audio_clear(); });
    expect(await page.evaluate(() => pc98.stats().error)).toBe('');
  }
  throw Error(`drawing guest exceeded 16 million slots: ${await page.evaluate(() => pc98.text())}`);
}
const shellReturned = page => page.evaluate(() =>
  pc98.text().includes('PROBE SHELL RETURN') && pc98.text().includes('A:\\>'));

test('Ap drawing timing: exact control, registers, BIOS state, repeated switching and reset in actual Wasm/FreeDOS', async ({ page }) => {
  test.setTimeout(60000);
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await boot(page, 'drawmode');
  await until(page, () => shellReturned(page));
  expect(await page.evaluate(() => pc98.text())).toContain('DRAWMODE RETURNED');
  expect(await page.evaluate(() => pc98.read(0x54d) & 8)).toBe(8);

  await boot(page, 'drawtest');
  for (let stage = 1; stage <= 4; stage++) {
    await until(page, () => page.evaluate(s => pc98.read(0x90000) === s, stage));
    const expected = stage === 1 || stage === 4 ? 8 : 0;
    expect(await page.evaluate(s => pc98.read(0x90000 + s) & 8, stage)).toBe(expected);
    expect(await page.evaluate(() => pc98.read(0x54d) & 8)).toBe(expected);
    await page.locator('#screen').focus();
    await page.keyboard.press('Enter');
  }
  await until(page, () => shellReturned(page));
  const text = await page.evaluate(() => pc98.text());
  expect(text).toContain('DRAW TIMING PASS');
  expect(text).not.toContain('FAILED');
  await page.evaluate(() => pc98.core.reset());
  expect(await page.evaluate(() => pc98.read(0x90000))).toBe(0);
  expect(await page.evaluate(() => pc98.read(0x54d) & 8)).toBe(0);
  await until(page, () => page.evaluate(() => pc98.read(0x90000) === 1));
  expect(await page.evaluate(() => pc98.read(0x54d) & 8)).toBe(8);
  await expect(page.locator('#error')).toBeHidden();
});
