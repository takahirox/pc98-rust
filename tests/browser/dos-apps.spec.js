import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

async function boot(page, name, sound = false) {
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  await page.locator('#profile').selectOption('1');
  if (sound) await page.locator('#sound').click();
  const buffer = await fs.readFile(new URL(`../../fixtures/dos-apps/${name}.hdm`, import.meta.url));
  await page.locator('#file').setInputFiles({ name: `${name}.hdm`, mimeType: 'application/octet-stream', buffer });
}
async function hash(page) {
  return page.evaluate(() => {
    const p = pc98.core.frame();
    const pixels = new Uint8Array(pc98.core.memory.buffer, p, 640 * 400 * 4);
    let h = 2166136261;
    for (const n of pixels) h = Math.imul(h ^ n, 16777619) >>> 0;
    return h;
  });
}
async function pixels(page, x, y, width, height) {
  return page.evaluate(({ x, y, width, height }) => {
    const p = pc98.core.frame(), data = new Uint8Array(pc98.core.memory.buffer, p, 640 * 400 * 4);
    return Array.from({ length: height }, (_, row) => Array.from({ length: width }, (_, col) =>
      Array.from(data.subarray(((y + row) * 640 + x + col) * 4, ((y + row) * 640 + x + col) * 4 + 3))));
  }, { x, y, width, height });
}
// Speed up the DOS boot with bounded real-Wasm slices, yielding every slice so
// the UI, Web Audio, errors and canvas still use their normal browser paths.
async function until(page, predicate, max = 220, budget = 100000, audio = false) {
  for (let i = 0; i < max; i++) {
    if (await predicate()) return;
    await page.evaluate(budget => pc98.core.step(budget), budget);
    if (audio) await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
    else await page.waitForTimeout(1);
    expect((await page.evaluate(() => pc98.stats())).error).toBe('');
  }
  throw Error('guest did not reach expected state within bounded instruction budget');
}
async function press(page, key) {
  await page.locator('#screen').focus();
  await page.keyboard.press(key);
  // Let the guest finish painting and return to its input loop.
  for (let i = 0; i < 20; i++) {
    await page.evaluate(() => pc98.core.step(100000));
    await page.waitForTimeout(1);
  }
  expect((await page.evaluate(() => pc98.stats())).error).toBe('');
}

test('Ap DOS: independent Japanese ADV game choices and guest save survive export/reimport', async ({ page }) => {
  test.setTimeout(120000);
  await boot(page, 'adv98');
  // The glyph is は (JIS 244F), independently rendered by the engine after CG
  // reads. Compare the whole 16x16 ink bitmap with the licensed font artifact.
  const font = await fs.readFile(new URL('../../fixtures/fonts/jis0208.bin', import.meta.url));
  const at = ((0x24 - 0x21) * 94 + 0x4f - 0x21) * 32;
  await until(page, async () => {
    const data = await pixels(page, 160, 317, 16, 16);
    return data.every((row, y) => row.every((rgb, x) =>
      rgb.every(n => n === 255) === Boolean(font[at + y * 2 + (x >> 3)] & (128 >> (x & 7)))));
  });
  // The first glyph can finish before the rest of the title is painted.
  // Complete that bounded guest drawing work before recording the stable frame.
  for (let i = 0; i < 20; i++) await page.evaluate(() => pc98.core.step(100000));
  const title = await hash(page);
  await press(page, 'Enter');
  expect(await hash(page)).not.toBe(title);
  const colors = (await pixels(page, 8, 100, 640 - 8, 1))[0];
  expect(new Set(colors.filter((_, x) => x % 40 === 0).map(rgb => rgb.join(','))).size).toBeGreaterThan(8);
  const dialogue = await hash(page);
  await press(page, 'Enter');
  expect(await hash(page)).not.toBe(dialogue);
  await press(page, 'ArrowRight'); await press(page, 'Enter');
  await press(page, 'Escape'); await press(page, 'Enter'); await press(page, 'Enter');
  const saved = Buffer.from(await page.evaluate(() => pc98.exportDisk()));
  expect(saved.includes(Buffer.from('ADV98SAV'))).toBe(true);
  expect((await page.evaluate(() => pc98.stats())).writes).toBeGreaterThan(0);
  await page.locator('#file').setInputFiles({ name: 'saved.hdm', mimeType: 'application/octet-stream', buffer: saved });
  await until(page, async () => (await hash(page)) === title);
  await press(page, 'ArrowRight'); await press(page, 'Enter'); await press(page, 'Enter');
  expect(await hash(page)).not.toBe(title);
  const again = Buffer.from(await page.evaluate(() => pc98.exportDisk()));
  expect(again.includes(Buffer.from('ADV98SAV'))).toBe(true);
  await expect(page.locator('#error')).toBeHidden();
  // Switching profiles resets CPU/devices while preserving mounted media.
  await page.locator('#profile').selectOption('0');
  await expect(page.locator('#display-mode')).toContainText('8 colors');
  await expect(page.locator('#media-name')).toHaveText('saved.hdm');
});

test('Ap DOS: independently compiled FM routines reach Web Audio with the beeper off', async ({ page }) => {
  test.setTimeout(60000);
  await boot(page, 'fm98', true);
  await until(page, async () => (await page.evaluate(() => pc98.text())).includes('INDEPENDENT FM ROUTINES OK'), 220, 100000, true);
  await expect.poll(() => page.evaluate(() => pc98.stats().audioEnergy)).toBeGreaterThan(0.1);
  const stats = await page.evaluate(() => pc98.stats());
  expect(stats.fmWrites).toBeGreaterThanOrEqual(40); expect(stats.fmKeyEdges).toBeGreaterThanOrEqual(16);
  expect(stats.beep).toBe(0); expect(stats.beepEdges).toBe(0);
  expect(stats.audioState).toBe('running'); expect(stats.scheduledSamples).toBeGreaterThan(1000);
  expect(stats.error).toBe('');
});

test('Ap DOS: independent video player writes color planes and returns to FreeCOM', async ({ page }) => {
  test.setTimeout(60000);
  // Freeze requestAnimationFrame before mounting the disk: otherwise animate()
  // can advance past transient video frames between our bounded steps/samples.
  // Other tests retain the live animation loop, including Web Audio coverage.
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await boot(page, '98videop');
  await expect(page.locator('#pause')).toHaveText('Pause');
  expect((await page.evaluate(() => pc98.stats())).instructions).toBe(0);
  const seen = new Set();
  await until(page, async () => {
    const rgb = (await pixels(page, 0, 0, 1, 1))[0][0]; seen.add(rgb.join(','));
    return (await page.evaluate(() => pc98.text())).includes('VIDEO RETURNED');
  }, 5000, 10000);
  expect(seen.has('0,255,255') || seen.has('255,119,119'), JSON.stringify([...seen])).toBe(true);
  expect([...seen].some(s => s.split(',').includes('119'))).toBe(true);
  expect((await page.evaluate(() => pc98.stats())).error).toBe('');
});

test('Ap DOS: independent CPULEVEL executes 386 operands and verifies 486 EFLAGS', async ({ page }) => {
  await boot(page, 'cpu386');
  await until(page, async () => (await page.evaluate(() => pc98.text())).includes('CPULEVEL 486 VERIFIED'));
  const stats = await page.evaluate(() => pc98.stats());
  const text = await page.evaluate(() => pc98.text());
  expect(text).toContain('i486 or compatible');
  expect(text).toContain('CPUID support check negative');
  expect(stats.reads).toBeGreaterThan(10);
  expect(await page.evaluate(() => pc98.core.metric(8))).toBeGreaterThanOrEqual(26);
  // Reset must clear upper registers/flags and diagnostic counters in the core.
  await page.evaluate(() => pc98.core.reset());
  expect(await page.evaluate(() => pc98.core.metric(8))).toBe(0);
  await until(page, async () => (await page.evaluate(() => pc98.text())).includes('CPULEVEL 486 VERIFIED'));
});
