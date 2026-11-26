import { bootInputProbe, diskAction, inputDisk, inputReady, inputState, uploadMedia } from './helpers/player.js';
import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';

async function ready(page) {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  return errors;
}
async function value(page, offset, expected) {
  await expect.poll(() => inspect(page, addr => pc98.read(addr), inputState + offset)).toBe(expected);
}

for (const profile of ['1', '0']) {
  test(`${profile === '1' ? 'Ap' : 'VM'}: Wasm guest boots, renders, plays PCM, handles keys and persists sector writes offline`, async ({ page }) => {
    const errors = await ready(page);
    if (profile === '0') await page.locator('#profile').selectOption(profile);
    await bootInputProbe(page);
    await expect.poll(() => inspect(page, () => pc98.text())).toContain(inputReady);
    await value(page, 0, 1);
    await expect.poll(() => inspect(page, () => pc98.stats().audioEnergy)).toBeGreaterThan(1);
    const state = await page.evaluate(async () => {
      const pixels = document.querySelector('#screen').getContext('2d').getImageData(0,0,640,16).data;
      let white = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i] === 255 && pixels[i+1] === 255 && pixels[i+2] === 255) white++;
      }
      return {...await pc98.stats(), white};
    });
    expect(state.reads).toBe(8);
    expect(state.beep).toBe(97);
    expect(state.beepEdges).toBe(1);
    expect(state.white).toBeGreaterThan(100);
    expect(state.audioState).toBe('running');
    await expect.poll(() => page.evaluate(async () => (await pc98.stats()).scheduledSamples)).toBeGreaterThan(1000);

    const unexpected = [];
    await page.route('**/*', route => { unexpected.push(route.request().url()); return route.abort(); });
    const screen = page.locator('#screen');
    const before = await screen.screenshot();
    await screen.focus();
    const scans = {ArrowRight:0x3c, ArrowDown:0x3d, ArrowUp:0x3a, ArrowLeft:0x3b};
    const keys = Object.keys(scans);
    for (let i = 0; i < 17; i++) {
      const key = keys[i % keys.length];
      await page.keyboard.press(key);
      await value(page, 2, i + 1);
      await value(page, 1, scans[key]);
    }
    expect(Buffer.compare(before, await screen.screenshot())).not.toBe(0);
    await expect.poll(() => inspect(page, () => pc98.stats().writes)).toBe(17);
    await page.locator('#pause').click();
    const paused = await inspect(page, () => pc98.stats().instructions);
    await page.waitForTimeout(100);
    expect(await inspect(page, () => pc98.stats().instructions)).toBe(paused);
    const download = page.waitForEvent('download');
    await diskAction(page, '0', 'export');
    const disk = await fs.readFile(await (await download).path());
    const expected = await inputDisk();
    expected[7 * 1024] = 17;
    expect(disk.equals(expected)).toBe(true);
    await uploadMedia(page, {name:'saved.hdm', mimeType:'application/octet-stream', buffer:disk});
    for (let i = 0; i < 2; i++) {
      await page.locator('#reset').click();
      await value(page, 0, 1);
      await value(page, 2, 0);
      await value(page, 4, 17);
    }
    await expect(page.locator('#media-name-0')).toHaveText('saved.hdm');
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
    expect(unexpected).toEqual([]);
  });
}

test('floppy validation, second drive, eject, and virtual PC-98 keyboard', async ({ page }) => {
  await ready(page);
  await bootInputProbe(page);
  await value(page, 0, 1);
  await uploadMedia(page, {name:'corrupt.d88', mimeType:'application/octet-stream', buffer:Buffer.alloc(100)});
  await expect(page.locator('#error')).toContainText('truncated D88');
  await expect(page.locator('#media-name-0')).toHaveText('input.hdm');
  await uploadMedia(page, {name:'blank.hdm', mimeType:'application/octet-stream', buffer:Buffer.alloc(1261568)}, '1');
  await expect(page.locator('#media-name-1')).toHaveText('blank.hdm');
  await diskAction(page, '1', 'eject');
  await expect(page.locator('#media-name-1')).toHaveText('Empty');
  await page.locator('#drive-1').click();
  await expect(page.locator('#export')).toBeHidden();
  await page.keyboard.press('Escape');
  await page.locator('#toggle-keyboard').click();
  const right = page.locator('[data-scan="60"]');
  await right.click(); await value(page, 2, 1);
  await right.evaluate(button => button.click()); await value(page, 2, 2);
  await expect.poll(() => inspect(page, () => pc98.read(0x531) & 0x10)).toBe(0);
  await right.press('Enter'); await value(page, 2, 3);
  await value(page, 1, 0x3c);
});

test('missing or broken Wasm fails visibly and leaves machine controls disabled', async ({page}) => {
  await page.route('**/core.wasm', route => route.fulfill({status:404, body:'missing'}));
  await page.goto('./');
  await expect(page.locator('#error')).toContainText('Missing Wasm bundle');
  await expect(page.locator('#profile')).toBeDisabled();
  expect(await page.evaluate(() => window.pc98)).toBeUndefined();
  await page.route('**/core.wasm', route => route.fulfill({status:200, body:'this is not WebAssembly'}));
  await page.reload();
  await expect(page.locator('#status')).toHaveText('Unable to start');
  await expect(page.locator('#profile')).toBeDisabled();
});
