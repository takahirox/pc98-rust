// Issue #120: production Wasm, original MIT guests only; no host guest results.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

const hello = await fs.readFile(new URL('../../web/media/hello98.tfd', import.meta.url));
const hdi = await fs.readFile(new URL('../../fixtures/ide_verify/ide_verify.hdi', import.meta.url));
const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/ide-verify-manifest.json', import.meta.url)));
expect(createHash('sha256').update(hdi).digest('hex')).toBe(manifest.image_sha256);

// Wrap the unchanged MIT HelloWorld-PC98 raw sectors in normal MFM D88 headers.
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
const wrapped = d88(hello);

async function ready(page) {
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  return errors;
}
async function upload(page, path, name, buffer) {
  if (path === 'picker') {
    await page.locator('#file').setInputFiles({ name, mimeType: 'application/octet-stream', buffer });
  } else {
    await page.locator('#drop').evaluate((drop, { name, base64 }) => {
      const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
      const dataTransfer = new DataTransfer();
      dataTransfer.items.add(new File([bytes], name));
      drop.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer }));
    }, { name, base64: buffer.toString('base64') });
  }
}
async function mounted(page, name, drive) {
  await expect(page.locator('#media-name')).toHaveText(name);
  await expect(page.locator('#media-drive')).toHaveText(drive);
  await expect(page.locator('#error')).toBeHidden();
}
async function boot(page, marker, drive) {
  const state = await page.evaluate(({ marker, budget }) => {
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
    expect(await page.evaluate(() => Array.from({ length: 1024 }, (_, i) => pc98.read(0x8000 + i))))
      .toEqual([...Buffer.from(manifest.result_hex, 'hex')]);
  } else expect(state.stats.reads).toBeGreaterThan(0);
}
async function exported(page, name, image) {
  const download = page.waitForEvent('download');
  await page.locator('#export').click();
  const file = await download;
  expect(file.suggestedFilename()).toBe(name);
  expect((await fs.readFile(await file.path())).equals(image)).toBe(true);
}

test('fresh page selects Ap in DOM and Rust, Auto, matching display and released input', async ({ page }) => {
  const errors = await ready(page);
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    await expect(page.locator('#profile')).toHaveValue('1');
    await expect(page).toHaveTitle('pc98-rust · PC-9821Ap DOS');
    await expect(page.locator('#drive')).toHaveValue('auto');
    await expect(page.locator('#profile-label')).toHaveText('PC-9821Ap DOS · experimental');
    await expect(page.locator('#display-mode')).toContainText('16 colors · Shinonome Japanese CG');
    await expect(page.locator('#mouse')).toBeEnabled();
    await expect(page.locator('#mouse')).not.toBeChecked();
    await expect(page.locator('#media-name')).toHaveText('No disk');
    await expect(page.locator('#eject')).toBeDisabled();
    await expect(page.locator('#export')).toBeDisabled();
    expect(await page.evaluate(() => ({ profile: pc98.core.metric(21), memory: pc98.read(0x401),
      keyboard: Array.from({ length: 16 }, (_, i) => pc98.read(0x52a + i)) })))
      .toEqual({ profile: 1, memory: 24, keyboard: Array(16).fill(0) });
  }
  expect(errors).toEqual([]);
});

for (const path of ['picker', 'drop']) {
  test(`Auto ${path}: every floppy extension boots real guest cold/reset, D88 exports intact`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    for (const extension of ['TfD', 'HDm', 'xDf', '2Hd', 'D88', 'd98', 'IMG']) {
      const image = /d88|d98/i.test(extension) ? wrapped : hello;
      const name = `hello.${extension}`;
      await upload(page, path, name, image);
      await mounted(page, name, 'Floppy 1');
      await expect(page.locator('#drive')).toHaveValue('auto');
      await expect(page.locator('#boot-source')).toHaveText(`Reset boots: Floppy 1 · ${name}`);
      await boot(page, 'Hello world!!', 0);
      await page.locator('#reset').click();
      await boot(page, 'Hello world!!', 0);
      if (extension === 'D88') await exported(page, name, image);
    }
    // Validate the other supported raw sizes through Rust, retaining exact bytes.
    // The existing synthetic IPL requests a 1024-byte boot sector; these are
    // mount/export controls, not claims of guest boot support for 512-byte sectors.
    for (const size of [80 * 2 * 15 * 512, 80 * 2 * 8 * 512]) {
      const image = Buffer.alloc(size);
      hello.copy(image, 0, 0, Math.min(size, hello.length));
      await upload(page, path, `geometry-${size}.img`, image);
      await mounted(page, `geometry-${size}.img`, 'Floppy 1');
      expect(await page.evaluate(() => pc98.read(0x584))).toBe(0x90);
      expect(await page.evaluate(() => pc98.stats().instructions)).toBe(0);
      await exported(page, `geometry-${size}.img`, image);
      await page.locator('#reset').click();
      await exported(page, `geometry-${size}.img`, image);
    }
    await page.locator('#eject').click();
    await expect(page.locator('#media-name')).toHaveText('No disk');
    await expect(page.locator('#media-drive')).toHaveText('Floppy 1');
    await expect(page.locator('#export')).toBeDisabled();
    expect(await page.evaluate(() => pc98.exportDisk(0))).toEqual([]);
    expect(errors).toEqual([]);
  });

  test(`Auto ${path}: HDI boots IDE cold/reset and media actions resolve to HDD`, async ({ page }) => {
    const errors = await ready(page);
    await upload(page, path, 'probe.HdI', hdi);
    await mounted(page, 'probe.HdI', 'IDE HDD');
    await expect(page.locator('#drive')).toHaveValue('auto');
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: IDE HDD · probe.HdI');
    await boot(page, 'IDE VERIFY PASS', 2);
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    await exported(page, 'probe.HdI', hdi);
    await page.locator('#eject').click();
    await expect(page.locator('#media-drive')).toHaveText('IDE HDD');
    await expect(page.locator('#media-name')).toHaveText('No disk');
    await expect(page.locator('#export')).toBeDisabled();
    expect(await page.evaluate(() => pc98.exportDisk(2))).toEqual([]);
    expect(errors).toEqual([]);
  });

  test(`manual ${path}: destinations, FDD2 no reboot, multiple disks, Auto reset and profile retention`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    await page.locator('#drive').selectOption('0');
    await upload(page, path, 'first.tfd', hello);
    await mounted(page, 'first.tfd', 'Floppy 1');
    await boot(page, 'Hello world!!', 0);
    await page.locator('#drive').selectOption('2');
    await upload(page, path, 'boot.hdi', hdi);
    await mounted(page, 'boot.hdi', 'IDE HDD');
    await boot(page, 'IDE VERIFY PASS', 2);
    const before = await page.evaluate(() => ({ stats: pc98.stats(), text: pc98.text() }));
    await page.locator('#drive').selectOption('1');
    await upload(page, path, 'second.tfd', hello);
    await mounted(page, 'second.tfd', 'Floppy 2');
    expect(await page.evaluate(() => ({ stats: pc98.stats(), text: pc98.text() }))).toEqual(before);
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: IDE HDD · boot.hdi');
    await exported(page, 'second.tfd', hello);
    await page.locator('#drive').selectOption('auto');
    await mounted(page, 'second.tfd', 'Floppy 2');
    await page.locator('#reset').click();
    await boot(page, 'IDE VERIFY PASS', 2);
    await page.locator('#eject').click();
    expect(await page.evaluate(() => pc98.exportDisk(1))).toEqual([]);
    await page.locator('#drive').selectOption('2');
    await exported(page, 'boot.hdi', hdi);
    await page.locator('#drive').selectOption('0');
    await exported(page, 'first.tfd', hello);
    await upload(page, path, 'floppy-boot.tfd', hello);
    await boot(page, 'Hello world!!', 0);
    await page.locator('#drive').selectOption('auto');
    await page.locator('#reset').click();
    await boot(page, 'Hello world!!', 0); // Mounted IDE must not steal boot.
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · floppy-boot.tfd');
    await page.locator('#drive').selectOption('2');
    await upload(page, path, 'boot.hdi', hdi);
    await boot(page, 'IDE VERIFY PASS', 2);
    await page.locator('#mouse').check();
    await page.locator('#screen').focus();
    await page.keyboard.down('ArrowRight');
    await page.locator('#profile').selectOption('0');
    await expect(page.locator('#profile')).toHaveValue('0');
    await expect(page).toHaveTitle('pc98-rust · PC-9801VM');
    await expect(page.locator('#profile-label')).toContainText('PC-9801VM');
    await expect(page.locator('#display-mode')).toContainText('8 colors');
    await expect(page.locator('#mouse')).toBeDisabled();
    await expect(page.locator('#mouse')).not.toBeChecked();
    expect(await page.evaluate(() => pc98.core.metric(21))).toBe(0);
    await expect(page.locator('#media-drive')).toHaveText('IDE HDD (requires Ap)');
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · floppy-boot.tfd');
    await exported(page, 'boot.hdi', hdi); // Switching profile never deletes IDE.
    await boot(page, 'Hello world!!', 0);
    expect(await page.evaluate(() => Array.from({ length: 16 }, (_, i) => pc98.read(0x52a + i))))
      .toEqual(Array(16).fill(0));
    await page.keyboard.up('ArrowRight');
    await page.locator('#profile').selectOption('1');
    await boot(page, 'IDE VERIFY PASS', 2); // Preferred source is retained.
    await page.locator('#drive').selectOption('auto');
    await mounted(page, 'boot.hdi', 'IDE HDD');
    await page.locator('#eject').click();
    await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · floppy-boot.tfd');
    await page.locator('#reset').click();
    await boot(page, 'Hello world!!', 0);
    expect(errors).toEqual([]);
  });

  test(`${path}: parser errors, limits and manual mismatches preserve media and profile`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = await ready(page);
    await upload(page, path, 'good.tfd', hello);
    await mounted(page, 'good.tfd', 'Floppy 1');
    const cases = [
      ['auto', 'bad.d88', Buffer.alloc(100), 'truncated D88'],
      ['auto', 'bad.d98', Buffer.alloc(0x2b0), 'D88 size mismatch'],
      ['auto', 'bad.img', Buffer.alloc(100), 'unsupported raw geometry'],
      ['auto', 'bad.hdi', Buffer.alloc(100), 'invalid HDI geometry'],
      ['auto', 'large.hdi', Buffer.alloc(5 * 1024 * 1024), 'invalid HDI geometry'],
      ['auto', 'large.hdm', Buffer.alloc(4 * 1024 * 1024 + 1), 'Floppy image exceeds 4 MiB'],
      ['auto', 'large.hdi', Buffer.alloc(32 * 1024 * 1024 + 1), 'HDI exceeds 32 MiB'],
      ['auto', 'program.exe', Buffer.alloc(100), 'Unsupported media extension'],
      ['0', 'wrong.hdi', hdi, 'HDI requires IDE HDD'],
      ['1', 'wrong.hdi', hdi, 'HDI requires IDE HDD'],
      ['2', 'wrong.tfd', hello, 'IDE HDD requires an HDI'],
    ];
    for (const [drive, name, image, error] of cases) {
      await page.locator('#drive').selectOption(drive);
      await upload(page, path, name, image);
      await expect(page.locator('#error')).toContainText(error);
      await expect(page.locator('#drive')).toHaveValue(drive);
      await expect(page.locator('#profile')).toHaveValue('1');
    }
    await page.locator('#drive').selectOption('auto');
    await expect(page.locator('#media-name')).toHaveText('good.tfd');
    await exported(page, 'good.tfd', hello);
    await page.locator('#profile').selectOption('0');
    for (const drive of ['auto', '0', '1', '2']) {
      await page.locator('#drive').selectOption(drive);
      await upload(page, path, 'ap-only.HDI', hdi);
      await expect(page.locator('#error')).toContainText('IDE requires PC-9821Ap DOS');
      await expect(page.locator('#profile')).toHaveValue('0');
      expect(await page.evaluate(() => pc98.core.metric(21))).toBe(0);
    }
    expect(errors).toEqual([]);
  });
}

for (const profile of ['1', '0']) {
  test(`${profile === '1' ? 'Ap' : 'VM'}: both bundled demos override either manual nonboot floppy choice or IDE cold/reset`, async ({ page }) => {
    const errors = await ready(page);
    await upload(page, 'picker', 'previous.hdi', hdi);
    await mounted(page, 'previous.hdi', 'IDE HDD');
    await boot(page, 'IDE VERIFY PASS', 2);
    if (profile === '0') await page.locator('#profile').selectOption(profile);
    for (const drive of ['1', '2']) {
      for (const [button, name, marker] of [['game', 'startrail.hdm', 'STAR TRAIL'], ['hello', 'hello98.tfd', 'Hello world!!']]) {
        await page.locator('#drive').selectOption(drive);
        await page.locator(`#${button}`).click();
        await mounted(page, name, 'Floppy 1');
        await expect(page.locator('#drive')).toHaveValue('0');
        await boot(page, marker, 0);
        await page.locator('#reset').click();
        await boot(page, marker, 0);
        await expect(page.locator('#profile')).toHaveValue(profile);
      }
    }
    expect(errors).toEqual([]);
  });
}
