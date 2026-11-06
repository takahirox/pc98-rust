import { inspect } from './helpers/runtime.js';
// Issue #126: production Rust/Wasm + unchanged GPL DOS + independent MIT data.
import { test, expect } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

const fixture = name => fs.readFile(new URL(`../../fixtures/${name}`, import.meta.url));
const boot = await fixture('freedos/minimal.hdm');
const data = await fixture('floppy_data/second.d88');
const replacement = await fixture('floppy_data/swapped.hdm');
const manifest = JSON.parse(await fixture('floppy-data-manifest.json'));
for (const [name, image] of [['second.d88', data], ['swapped.hdm', replacement]]) {
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.images[name]);
}

async function until(page, expected) {
  const state = await inspect(page, expected => {
    for (let slots = 0; slots < 8_000_000; slots += 10000) {
      if (pc98.text().includes(expected) || pc98.stats().error) break;
      pc98.advance(10000);
      pc98.core.audio_clear();
    }
    return { text: pc98.text(), stats: pc98.stats() };
  }, expected);
  expect(state.stats.error).toBe('');
  expect(state.text).toContain(expected);
  return state.text;
}
async function keys(page, text) {
  await page.locator('#screen').focus();
  for (const char of text) {
    // Physical browser events go through app.js's normal PC-98 key map.
    const code = char === ':' ? 'Quote' : char === '.' ? 'Period' :
      char === ' ' ? 'Space' : char === '\r' ? 'Enter' : `Key${char.toUpperCase()}`;
    await page.keyboard.down(code);
    await inspect(page, () => { pc98.advance(50000); pc98.core.audio_clear(); });
    await page.keyboard.up(code);
    await inspect(page, () => { pc98.advance(10000); pc98.core.audio_clear(); });
  }
}
const command = (page, text) => keys(page, `${text}\r`);
async function mount(page, name, buffer) {
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
  await expect(page.locator('#media-name')).toHaveText(name);
  await expect(page.locator('#media-drive')).toHaveText('Floppy 2');
  await expect(page.locator('#error')).toBeHidden();
}
async function readFile(page, name, contents, requireRead = true) {
  await command(page, 'cls');
  const reads = await inspect(page, () => pc98.stats().reads);
  await command(page, `type ${name}`);
  await until(page, contents);
  if (requireRead) expect(await inspect(page, () => pc98.stats().reads)).toBeGreaterThan(reads);
  await until(page, 'B:\\>');
}
async function exported(page, name, bytes) {
  const pending = page.waitForEvent('download');
  await page.locator('#export').click();
  const download = await pending;
  expect(download.suggestedFilename()).toBe(name);
  expect((await fs.readFile(await download.path())).equals(bytes)).toBe(true);
}

test('Ap DOS enumerates empty FDD2, reads post-boot D88, swaps/ejects/reinserts and reports no media', async ({ page }) => {
  test.setTimeout(120000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#profile')).toHaveValue('1');
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'minimal.hdm', mimeType: 'application/octet-stream', buffer: boot });
  await expect(page.locator('#media-drive')).toHaveText('Floppy 1');
  await inspect(page, () => pc98.core.set_boot_source(0));
  await page.locator('#reset').click();
  await until(page, 'A:\\>');
  expect(await inspect(page, () => [pc98.read(0x55c), pc98.read(0x66c), pc98.read(0x66d)]))
    .toEqual([3, 0x90, 0x91]);
  await page.locator('#drive').selectOption('1');
  const before = await inspect(page, () => ({ stats: pc98.stats(), text: pc98.text() }));
  await mount(page, 'second.d88', data);
  expect(await inspect(page, () => ({ stats: pc98.stats(), text: pc98.text() }))).toEqual(before);
  await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · minimal.hdm');
  await command(page, 'b:');
  await until(page, 'B:\\>');
  await command(page, 'dir');
  await until(page, 'SECOND');
  await readFile(page, 'second.txt', 'ORIGINAL FDD2 FILE CONTENTS');
  await readFile(page, 'second.txt', 'ORIGINAL FDD2 FILE CONTENTS', false);
  await exported(page, 'second.d88', data);
  await mount(page, 'swapped.hdm', replacement);
  await command(page, 'cls');
  await command(page, 'dir');
  expect(await until(page, 'SWAPPED')).not.toContain('SECOND');
  await readFile(page, 'swapped.txt', 'REPLACEMENT FDD2 FILE CONTENTS');
  await exported(page, 'swapped.hdm', replacement);
  await page.locator('#eject').click();
  await expect(page.locator('#media-name')).toHaveText('No disk');
  await expect(page.locator('#export')).toBeDisabled();
  expect(await inspect(page, () => pc98.read(0x55c))).toBe(3);
  await mount(page, 'second.d88', data);
  await command(page, 'cls');
  await command(page, 'dir');
  await until(page, 'SECOND');
  await readFile(page, 'second.txt', 'ORIGINAL FDD2 FILE CONTENTS');
  const stats = await inspect(page, () => pc98.stats());
  expect(stats.writes).toBe(0);
  expect(stats.cycles).toBeLessThan(12_000_000*8);
  console.log(`FDD2 live insert/swap/eject/reinsert: ${stats.instructions} slots`);
  // The same D88 mounted before reset is the user's working control.
  await page.locator('#reset').click();
  await until(page, 'A:\\>');
  await command(page, 'b:');
  await until(page, 'B:\\>');
  await command(page, 'dir');
  await until(page, 'SECOND');
  await readFile(page, 'second.txt', 'ORIGINAL FDD2 FILE CONTENTS');
  expect((await inspect(page, () => pc98.stats())).writes).toBe(0);
  await exported(page, 'second.d88', data);
  await page.locator('#drive').selectOption('0');
  await exported(page, 'minimal.hdm', boot);
  await page.locator('#drive').selectOption('1');
  await page.locator('#eject').click();
  await command(page, 'cls');
  await command(page, 'dir');
  expect(await until(page, '(R)')).not.toContain('SECOND');
  expect(await inspect(page, () => pc98.read(0x55c))).toBe(3);
  expect(errors).toEqual([]);
});
