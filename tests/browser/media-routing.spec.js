import { diskAction, inputDisk, nextBoot, uploadMedia } from './helpers/player.js';
import { inspect, mediaGuestState } from './helpers/runtime.js';
// Issue #120: production Wasm, original MIT guests only; no host guest results.
import { test, expect } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

const floppy = await inputDisk();
const hdi = await fs.readFile(new URL('../../fixtures/ide_verify/ide_verify.hdi', import.meta.url));
const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/ide-verify-manifest.json', import.meta.url)));
expect(createHash('sha256').update(hdi).digest('hex')).toBe(manifest.image_sha256);

// Wrap the MIT input probe’s raw sectors in normal MFM D88 headers.
function d88(raw) {
  const image = Buffer.alloc(0x2b0 + 154 * 8 * (16 + 1024));
  image[0x1a] = 1; // Preserve write protection through export.
  image[0x1b] = 0x20;
  image.writeUInt32LE(image.length, 0x1c);
  let at = 0x2b0;
  for (let track = 0; track < 154; track++) {
    image.writeUInt32LE(at, 0x20 + track * 4);
    for (let sector = 0; sector < 8; sector++) {
      image.set([track >> 1, track & 1, sector + 1, 3, 8, 0], at);
      image.writeUInt16LE(1024, at + 14);
      raw.copy(image, at + 16, (track * 8 + sector) * 1024, (track * 8 + sector + 1) * 1024);
      at += 16 + 1024;
    }
  }
  return image;
}
const wrapped = d88(floppy);

async function ready(page) {
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  return errors;
}
async function upload(page, path, name, buffer, drive) {
  if (path === 'picker') {
    await uploadMedia(page, { name, mimeType: 'application/octet-stream', buffer },
      drive ?? (/\.(hdi|nhd)$/i.test(name) ? '2' : '0'));
  } else {
    await page.locator(drive === undefined ? '#drop' : `#slot-${drive}`).evaluate((drop, { name, base64 }) => {
      const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
      const dataTransfer = new DataTransfer();
      dataTransfer.items.add(new File([bytes], name));
      drop.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer }));
    }, { name, base64: buffer.toString('base64') });
  }
  // Event dispatch returns before chunked HDI import and the guest reset finish.
  await expect(page.locator('#profile')).toBeEnabled();
}
async function expectEmpty(page, drive) {
  await diskAction(page, drive, 'eject');
  await expect(page.locator(`#media-name-${drive}`)).toHaveText('Empty');
  if (drive === '2') {
    await expect(page.locator('#hdd-actions')).toBeHidden();
    return;
  }
  await page.locator(`#drive-${drive}`).click();
  await expect(page.locator('#export')).toBeHidden();
  await page.keyboard.press('Escape');
}

async function mounted(page, name, drive) {
  const slot = page.getByRole('group', {name: drive, exact: true});
  await expect(slot.locator('.media-name')).toHaveText(name);
  await expect(slot.getByRole('button', {name: `Change ${drive}`})).toBeEnabled();
  await expect(page.locator('#error')).toBeHidden();
}
async function boot(page, marker, drive) {
  const state = await inspect(page, ({ marker, budget }) => {
    let slots = 0;
    while (slots < budget && !pc98.text().includes(marker) && !pc98.stats().error) {
      slots += pc98.core.step(1000);
      pc98.core.audio_clear();
    }
    return { text: pc98.text(), stats: pc98.stats(), boot: pc98.read(0x584) };
  }, { marker, budget: 500000 });
  expect(state.text).toContain(marker);
  expect(state.text).not.toContain('FAIL');
  expect(state.stats.error).toBe('');
  expect(state.boot).toBe(drive === 2 ? 0x80 : 0x90);
  if (drive === 2) {
    expect(state.stats.hddReads).toBe(17);
    expect(state.stats.reads).toBe(0);
    expect(await inspect(page, () => Array.from({ length: 1024 }, (_, i) => pc98.read(0x8000 + i))))
      .toEqual([...Buffer.from(manifest.result_hex, 'hex')]);
  } else expect(state.stats.reads).toBeGreaterThan(0);
}
async function exported(page, name, image, drive = /\.(hdi|nhd)$/i.test(name) ? '2' : '0') {
  const download = page.waitForEvent('download');
  await diskAction(page, drive, 'export');
  const file = await download;
  expect(file.suggestedFilename()).toBe(name);
  expect((await fs.readFile(await file.path())).equals(image)).toBe(true);
}

test('Ap mounts and boots an NHD, retains it after a malformed replacement, and exports its container', async ({ page }) => {
  await ready(page);
  const header = Buffer.alloc(512);
  header.write('T98HDDIMAGE.R0');
  header.write('IDE test in an NHD container', 16);
  header.writeUInt32LE(header.length, 0x110);
  header.writeUInt32LE(hdi.readUInt32LE(28), 0x114);
  header.writeUInt16LE(hdi.readUInt32LE(24), 0x118);
  header.writeUInt16LE(hdi.readUInt32LE(20), 0x11a);
  header.writeUInt16LE(512, 0x11c);
  const image = Buffer.concat([header, hdi.subarray(hdi.readUInt32LE(8))]);
  await upload(page, 'picker', 'boot.NHD', image);
  await mounted(page, 'boot.NHD', 'IDE HDD');
  await boot(page, 'IDE VERIFY PASS', 2);
  await upload(page, 'picker', 'truncated.nhd', image.subarray(0, image.length - 1));
  await expect(page.locator('#error')).toContainText('invalid NHD geometry');
  await expect(page.locator('#media-name-2')).toHaveText('boot.NHD');
  await exported(page, 'boot.NHD', image);
});

test('fresh page selects Ap in DOM and Rust, matching display and released input', async ({ page }) => {
  const errors = await ready(page);
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    await expect(page.locator('#profile')).toHaveValue('1');
    await expect(page).toHaveTitle('pc98-rust · PC-9821Ap DOS');
    await expect(page.locator('#profile option:checked')).toHaveText('PC-9821Ap');
    await expect(page.locator('#display-mode')).toContainText('16 colors · Shinonome Japanese CG');
    await expect(page.locator('#media-name-2')).toHaveText('FreeDOS-128MiB.hdi');
    await page.locator('#hdd-actions').click();
    const actions = page.getByRole('dialog', {name:'HDD actions'});
    await expect(actions.getByRole('button', {name:'Eject'})).toBeEnabled();
    await expect(actions.getByRole('button', {name:'Export disk'})).toBeEnabled();
    await page.keyboard.press('Escape');
    expect(await inspect(page, () => ({ profile: pc98.core.metric(21), memory: pc98.read(0x401),
      keyboard: Array.from({ length: 16 }, (_, i) => pc98.read(0x52a + i)) })))
      .toEqual({ profile: 1, memory: 24, keyboard: Array(16).fill(0) });
  }
  expect(errors).toEqual([]);
});

test('dropping a replacement HDD on Display closes HDD actions and returns focus to the screen', async ({ page }) => {
  await ready(page);
  const actions = page.getByRole('dialog', {name: 'HDD actions'});
  await page.getByRole('button', {name: 'HDD actions'}).press('Enter');
  await expect(actions.getByRole('button', {name: 'Export disk'})).toBeFocused();

  await upload(page, 'drop', 'replacement.hdi', hdi);

  await mounted(page, 'replacement.hdi', 'IDE HDD');
  await expect(actions).toBeHidden();
  await expect(page.getByRole('img', {name: 'PC-98 screen'})).toBeFocused();
});

for (const path of ['picker', 'drop']) {
  test(`${path}: every floppy extension mounts live then explicitly boots real guest cold/reset, D88 exports intact`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    for (const extension of ['TfD', 'HDm', 'xDf', '2Hd', 'D88', 'd98', 'IMG']) {
      const image = /d88|d98/i.test(extension) ? wrapped : floppy;
      const name = `floppy.${extension}`;
      await upload(page, path, name, image);
      await mounted(page, name, 'Floppy 1');
      await nextBoot(page, '0');
      await page.locator('#reset').click();
      await expect(page.locator('#boot-source')).toHaveText(`Reset boots: Floppy 1 · ${name}`);
      await boot(page, 'INPUT PROBE READY', 0);
      await page.locator('#reset').click();
      await boot(page, 'INPUT PROBE READY', 0);
      if (extension === 'D88') await exported(page, name, image);
    }
    // Validate the other supported raw sizes through Rust, retaining exact bytes.
    // The existing synthetic IPL requests a 1024-byte boot sector; these are
    // mount/export controls, not claims of guest boot support for 512-byte sectors.
    for (const size of [80 * 2 * 15 * 512, 80 * 2 * 8 * 512]) {
      const image = Buffer.alloc(size);
      floppy.copy(image, 0, 0, Math.min(size, floppy.length));
      await upload(page, path, `geometry-${size}.img`, image);
      await mounted(page, `geometry-${size}.img`, 'Floppy 1');
      expect(await inspect(page, () => pc98.read(0x584))).toBe(0x90);
      await page.locator('#reset').click();
      expect(await inspect(page, () => pc98.stats().instructions)).toBe(0);
      await exported(page, `geometry-${size}.img`, image);
      await page.locator('#reset').click();
      await exported(page, `geometry-${size}.img`, image);
    }
    await expectEmpty(page, '0');
    expect(await inspect(page, () => pc98.exportDisk(0))).toEqual([]);
    expect(errors).toEqual([]);
  });

  test(`${path}: HDI boots IDE cold/reset and media actions resolve to HDD`, async ({ page }) => {
    const errors = await ready(page);
    await upload(page, path, 'probe.HdI', hdi);
    await mounted(page, 'probe.HdI', 'IDE HDD');
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: IDE HDD · probe.HdI');
    await boot(page, 'IDE VERIFY PASS', 2);
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    await exported(page, 'probe.HdI', hdi);
    await expectEmpty(page, '2');
    expect(await inspect(page, () => pc98.exportDisk(2))).toEqual([]);
    expect(errors).toEqual([]);
  });

  test(`manual ${path}: destinations, FDD2 no reboot, multiple disks, explicit reset and profile retention`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    await upload(page, path, 'first.tfd', floppy, '0');
    await mounted(page, 'first.tfd', 'Floppy 1');
    await nextBoot(page, '0');
    await page.locator('#reset').click();
    await boot(page, 'INPUT PROBE READY', 0);
    await upload(page, path, 'boot.hdi', hdi, '2');
    await mounted(page, 'boot.hdi', 'IDE HDD');
    await nextBoot(page, '1');
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    const before = await mediaGuestState(page);
    await upload(page, path, 'second.tfd', floppy, '1');
    await mounted(page, 'second.tfd', 'Floppy 2');
    expect(await mediaGuestState(page)).toEqual(before);
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: IDE HDD · boot.hdi');
    await exported(page, 'second.tfd', floppy, '1');
    await mounted(page, 'second.tfd', 'Floppy 2');
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    await diskAction(page, '1', 'eject');
    expect(await inspect(page, () => pc98.exportDisk(1))).toEqual([]);
    await exported(page, 'boot.hdi', hdi);
    await exported(page, 'first.tfd', floppy);
    await upload(page, path, 'floppy-boot.tfd', floppy, '0');
    await nextBoot(page, '0');
    await page.locator('#reset').click();
    await boot(page, 'INPUT PROBE READY', 0);
    await page.locator('#reset').click();
    await boot(page, 'INPUT PROBE READY', 0); // Mounted IDE must not steal boot.
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · floppy-boot.tfd');
    await upload(page, path, 'boot.hdi', hdi, '2');
    await nextBoot(page, '1');
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    // The IDE probe halts with interrupts disabled. Boot the mounted floppy
    // to exercise BIOS keyboard handling while retaining the HDD for Ap.
    await nextBoot(page, '0');
    await page.locator('#reset').click();
    await boot(page, 'INPUT PROBE READY', 0);
    await nextBoot(page, '1');
    await page.locator('#screen').focus();
    await page.keyboard.down('ArrowRight');
    expect(await inspect(page, () => {
      pc98.advance(10000);
      return pc98.read(0x531) & 0x10; // BIOS bitmap entry for scan 0x3c.
    })).toBe(0x10);
    await page.locator('#profile').selectOption('0');
    await expect(page.locator('#profile')).toHaveValue('0');
    await expect(page).toHaveTitle('pc98-rust · PC-9801VM');
    await expect(page.locator('#profile option:checked')).toHaveText('PC-9801VM');
    await expect(page.locator('#display-mode')).toContainText('8 colors');
    expect(await inspect(page, () => pc98.core.metric(21))).toBe(0);
    await expect(page.locator('#media-name-2')).toHaveText('Requires Ap · boot.hdi');
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · floppy-boot.tfd');
    await exported(page, 'boot.hdi', hdi); // Switching profile never deletes IDE.
    await boot(page, 'INPUT PROBE READY', 0);
    expect(await inspect(page, () => Array.from({ length: 16 }, (_, i) => pc98.read(0x52a + i))))
      .toEqual(Array(16).fill(0));
    await page.keyboard.up('ArrowRight');
    await page.locator('#profile').selectOption('1');
    await boot(page, 'IDE VERIFY PASS', 2); // Preferred source is retained.
    await mounted(page, 'boot.hdi', 'IDE HDD');
    await nextBoot(page, '1');
    await page.locator('#reset').click();
    await diskAction(page, '2', 'eject');
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: IDE HDD · Empty');
    await page.locator('#reset').click();
    // No implicit fallback to the mounted floppy.
    await inspect(page, () => pc98.core.step(10000));
    expect(await inspect(page, () => pc98.stats().reads)).toBe(0);
    await nextBoot(page, '0');
    await page.locator('#reset').click();
    await boot(page, 'INPUT PROBE READY', 0);
    expect(errors).toEqual([]);
  });

  test(`${path}: parser errors, limits and manual mismatches preserve media and profile`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    await upload(page, path, 'good.tfd', floppy);
    await mounted(page, 'good.tfd', 'Floppy 1');
    const cases = [
      [undefined, 'bad.d88', Buffer.alloc(100), 'truncated D88'],
      [undefined, 'bad.d98', Buffer.alloc(0x2b0), 'D88 size mismatch'],
      [undefined, 'bad.img', Buffer.alloc(100), 'unsupported raw geometry'],
      [undefined, 'bad.hdi', Buffer.alloc(100), 'invalid HDI geometry'],
      [undefined, 'large.hdi', Buffer.alloc(5 * 1024 * 1024), 'invalid HDI geometry'],
      [undefined, 'large.hdm', Buffer.alloc(4 * 1024 * 1024 + 1), 'Floppy image exceeds 4 MiB'],
      [undefined, 'large.hdi', Buffer.alloc(32 * 1024 * 1024 + 1), 'invalid HDI geometry'],
      [undefined, 'program.exe', Buffer.alloc(100), 'Unsupported media extension'],
      ['0', 'wrong.hdi', hdi, 'Hard disk images require IDE HDD'],
      ['1', 'wrong.hdi', hdi, 'Hard disk images require IDE HDD'],
      ['2', 'wrong.tfd', floppy, 'IDE HDD requires an HDI'],
    ];
    for (const [drive, name, image, error] of cases) {
      await upload(page, path, name, image, drive);
      await expect(page.locator('#error')).toContainText(error);
      await expect(page.locator('#profile')).toHaveValue('1');
    }
    await expect(page.locator('#media-name-0')).toHaveText('good.tfd');
    await exported(page, 'good.tfd', floppy);
    await page.locator('#profile').selectOption('0');
    const drives = path === 'drop' ? [undefined, '0', '1', '2'] : ['0', '1', '2'];
    for (const drive of drives) {
      if (path === 'picker' && drive === '2') {
        await page.locator('#drive-2').click();
        await expect(page.locator('#choose-file')).toBeDisabled();
        await expect(page.locator('#media-hint')).toContainText('Switch to PC-9821Ap');
        await page.keyboard.press('Escape');
      } else {
        await upload(page, path, 'ap-only.HDI', hdi, drive);
        await expect(page.locator('#error')).toContainText('IDE requires PC-9821Ap DOS');
      }
      await expect(page.locator('#profile')).toHaveValue('0');
      expect(await inspect(page, () => pc98.core.metric(21))).toBe(0);
    }
    expect(errors).toEqual([]);
  });
}
