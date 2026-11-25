import { readFile } from 'node:fs/promises';

// Use the same picker, chooser and settings controls as a player.
export async function uploadMedia(page, files, drive = '0') {
  await page.locator(`#drive-${drive}`).click();
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.locator('#choose-file').click(),
  ]);
  await chooser.setFiles(files);
}

export async function diskAction(page, drive, action) {
  if (Number(drive) === 2) {
    await page.locator('#hdd-actions').click();
    await page.locator(action === 'recover-hdd' ? '#recover-hdd' : `#${action}-hdd`).click();
    return;
  }
  await page.locator(`#drive-${drive}`).click();
  await page.locator(`#${action}`).click();
}

export async function openSettings(page) {
  if (!await page.locator('#settings-panel').isVisible()) await page.locator('#settings').click();
}

export async function nextBoot(page, source) {
  await openSettings(page);
  await page.locator('#boot').selectOption(source);
  await page.keyboard.press('Escape');
}

export const inputDisk = () => readFile(new URL('../../../fixtures/input_probe/input.hdm', import.meta.url));
export const inputReady = 'INPUT PROBE READY';
export const inputState = 0x1ff00;

export async function bootInputProbe(page, { touch = false, sound = false } = {}) {
  await uploadMedia(page, { name:'input.hdm', mimeType:'application/octet-stream', buffer:await inputDisk() }, '0');
  if (await page.locator('#profile').inputValue() === '1') await nextBoot(page, '0');
  await page.locator('#reset')[touch ? 'tap' : 'click']();
  if (sound) await page.locator('#sound').click();
}
