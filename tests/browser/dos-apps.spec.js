import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';

async function boot(page, name, sound = false, extension = 'hdm') {
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  await page.locator('#profile').selectOption('1');
  if (extension === 'hdi') await page.locator('#drive').selectOption({value: '2'});
  if (sound) await page.locator('#sound').click();
  const buffer = await fs.readFile(new URL(`../../fixtures/dos-apps/${name}.${extension}`, import.meta.url));
  await page.locator('#file').setInputFiles({ name: `${name}.${extension}`, mimeType: 'application/octet-stream', buffer });
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

async function waitTitle(page) {
  const font = await fs.readFile(new URL('../../fixtures/fonts/jis0208.bin', import.meta.url));
  const at = ((0x24 - 0x21) * 94 + 0x4f - 0x21) * 32;
  await until(page, async () => {
    const data = await pixels(page, 160, 317, 16, 16);
    return data.every((row, y) => row.every((rgb, x) =>
      rgb.every(n => n === 255) === Boolean(font[at + y * 2 + (x >> 3)] & (128 >> (x & 7)))));
  }, 260);
  for (let i = 0; i < 20; i++) await page.evaluate(() => pc98.core.step(100000));
}

function hdiFile(image, name) {
  const volume = image.subarray(4096 + 128 * 512);
  const root = volume.subarray(14 * 512, 26 * 512);
  let entry;
  for (let i = 0; i < root.length; i += 32) {
    if (root.subarray(i, i + 11).toString() === name) { entry = root.subarray(i, i+32); break; }
  }
  expect(entry, `guest-created HDD file ${name}`).toBeDefined();
  const chunks = [];
  let cluster = entry.readUInt16LE(26);
  for (let i = 0; i < 2010 && cluster < 0xff8; i++) {
    expect(cluster).toBeGreaterThanOrEqual(2); expect(cluster).toBeLessThan(2010);
    const at = (26 + (cluster - 2) * 4) * 512;
    chunks.push(volume.subarray(at, at + 2048));
    cluster = (volume.readUInt16LE(1024 + Math.floor(cluster * 3 / 2)) >>> (cluster & 1 ? 4 : 0)) & 0xfff;
  }
  return Buffer.concat(chunks).subarray(0, entry.readUInt32LE(28));
}

async function exportHdd(page) {
  // Transfer the full image as one base64 string. Serializing millions of
  // individual numbers through Playwright can exhaust the CI timeout.
  const encoded = await page.evaluate(() => {
    const bytes = pc98.exportDisk(2);
    let binary = '';
    for (let at = 0; at < bytes.length; at += 32768) {
      binary += String.fromCharCode(...bytes.slice(at, at + 32768));
    }
    return btoa(binary);
  });
  return Buffer.from(encoded, 'base64');
}

test('Ap IDE HDD boots DOS and ADV98; mouse reaches IRQ driver and game; export preserves DOS saves', async ({ page }) => {
  test.setTimeout(120000);
  await boot(page, 'adv98', false, 'hdi');
  await waitTitle(page);
  const title = await hash(page);
  let stats = await page.evaluate(() => pc98.stats());
  expect(stats.hddReads).toBeGreaterThan(100); expect(stats.hddWrites).toBeGreaterThan(0);
  expect(stats.reads).toBe(0);
  const state = await page.evaluate(() => {
    const signature = [...'M98STATE'].map(c => c.charCodeAt(0));
    for (let i = 0x600; i < 0xa0000; i++) {
      if (signature.every((v, j) => pc98.read(i + j) === v)) return i;
    }
    throw Error('resident mouse driver not found');
  });
  await page.locator('#mouse').check();
  const box = await page.locator('#screen').boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.move(box.x + box.width * (352 / 640), box.y + box.height * (184 / 400));
  await page.mouse.down();
  await until(page, async () => await page.evaluate(state => pc98.read(state + 12) === 1, state), 20);
  expect(await page.evaluate(state => pc98.read(state+8) | pc98.read(state+9) << 8, state)).toBe(352);
  expect(await page.evaluate(state => pc98.read(state+10) | pc98.read(state+11) << 8, state)).toBe(184);
  await page.mouse.up();
  await until(page, async () => (await hash(page)) !== title, 50);
  // Movement can choose a different title entry. Reboot before the separate
  // keyboard save workflow so its initial menu selection is deterministic.
  await page.locator('#reset').click(); await waitTitle(page);
  await press(page, 'Enter');
  await press(page, 'Enter'); await press(page, 'ArrowRight'); await press(page, 'Enter');
  await press(page, 'Escape'); await press(page, 'Enter'); await press(page, 'Enter');
  const saved = await exportHdd(page);
  // A save created by the independent game's DOS I/O, absent from initial HDI.
  const save = hdiFile(saved, 'SAVE1   DAT');
  expect(save.subarray(0,8).toString()).toBe('ADV98SAV');
  expect(hdiFile(saved, 'HDDSAVE TXT').toString()).toBe('DURABLE DOS WRITE\r\n');
  expect((await page.evaluate(() => pc98.stats())).mouseLatches).toBeGreaterThan(0);
  await page.locator('#file').setInputFiles({ name: 'saved.hdi', mimeType: 'application/octet-stream', buffer: saved });
  await waitTitle(page);
  await press(page, 'ArrowRight'); await press(page, 'Enter'); await press(page, 'Enter');
  expect(await hash(page)).not.toBe(title);
  expect(hdiFile(await exportHdd(page), 'SAVE1   DAT')).toEqual(save);
  await expect(page.locator('#error')).toBeHidden();
  // Rejected geometry must preserve the mounted disk and avoid a Wasm trap.
  const bad = Buffer.from(saved); bad.writeUInt32LE(0xffffffff, 28);
  await page.locator('#file').setInputFiles({ name: 'bad.hdi', mimeType: 'application/octet-stream', buffer: bad });
  await expect(page.locator('#error')).toContainText('invalid HDI geometry');
  await expect(page.locator('#media-name')).toHaveText('saved.hdi');
  expect(hdiFile(await exportHdd(page), 'SAVE1   DAT')).toEqual(save);
});

test('Ap DOS: unchanged ADV98 starts sequenced PMD FM music into actual Web Audio', async ({ page }) => {
  test.setTimeout(120000);
  await boot(page, 'music', true); await waitTitle(page);
  const before = await page.evaluate(() => pc98.stats());
  expect(before.fmKeyEdges).toBe(0); expect(before.audioEnergy).toBe(0);
  await press(page, 'Enter');
  await until(page, async () => (await page.evaluate(() => pc98.stats())).fmKeyEdges > 24, 200, 100000, true);
  const stats = await page.evaluate(() => pc98.stats());
  expect(stats.audioState).toBe('running'); expect(stats.audioEnergy).toBeGreaterThan(1);
  expect(stats.scheduledSamples).toBeGreaterThan(1000); expect(stats.fmWrites).toBeGreaterThan(500);
  expect(stats.beepEdges).toBe(0); expect(stats.error).toBe('');
});

test('Ap DOS: independent 98VIDEOP uses 86 PCM FIFO and Web Audio', async ({ page }) => {
  await boot(page, '98videop', true);
  await until(page, async () => (await page.evaluate(() => pc98.text())).includes('VIDEO RETURNED'), 300, 100000, true);
  const stats = await page.evaluate(() => pc98.stats());
  expect(stats.pcmWrites).toBeGreaterThan(100); expect(stats.pcmSamples).toBeGreaterThan(100);
  expect(stats.beepEdges).toBe(0); expect(stats.audioEnergy).toBeGreaterThan(0.01);
  expect(stats.audioState).toBe('running'); expect(stats.scheduledSamples).toBeGreaterThan(1000);
  expect(stats.error).toBe('');
});

test('Ap DOS: independent DEBUG drives EGC, PEGC banks and a 256-color framebuffer', async ({ page }) => {
  test.setTimeout(120000);
  await boot(page, 'pegc');
  await until(page, async () => (await page.evaluate(() => pc98.text())).includes('PEGC UTILITY COMPLETE'), 260);
  expect(await page.evaluate(() => pc98.text())).toContain('EXTENDED MEMORY OK');
  expect(await page.evaluate(() => Array.from({length:16}, (_,i) => pc98.read(0x200000+i))))
    .toEqual([...Buffer.from('PC98 INT1F RAM!!')]);
  expect(await page.evaluate(() => Array.from({length:64}, (_,i) => pc98.read(0x90000+i))))
    .toEqual([...Array(48).fill(255), ...Array(16).fill(0)]);
  const data = await pixels(page, 0, 0, 640, 50);
  for (let y = 0; y < 50; y++) for (let x = 0; x < 640; x += 128) {
    const color = Math.floor((y * 640 + x) / 128);
    expect(data[y][x]).toEqual([color, 255-color, color ^ 85]);
  }
  await expect(page.locator('#display-mode')).toContainText('256 colors');
  expect((await page.evaluate(() => pc98.stats())).error).toBe('');
});

for (const profile of ['0', '1']) {
  test(`DOS timer callback: guest IRQ frame, IRET and one-shot flag (profile ${profile})`, async ({ page }) => {
    // Freeze animation so the asserted maximum is the actual Wasm step budget.
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const buffer = await fs.readFile(new URL('../../fixtures/freedos/timer.hdm', import.meta.url));
    await page.locator('#file').setInputFiles({ name: 'timer.hdm', mimeType: 'application/octet-stream', buffer });
    await until(page, async () => (await page.evaluate(() => pc98.text())).includes('TIMER CALLBACK PASS'), 160);
    const state = await page.evaluate(() => {
      const signature = [...'T98STATE'].map(c => c.charCodeAt(0));
      for (let p = 0x600; p < 0x9ffe0; p++) {
        if (signature.every((v, j) => pc98.read(p + j) === v) && pc98.read(p + 9) === 1) return p;
      }
      throw Error('guest did not validate the callback frame');
    });
    const values = await page.evaluate(p => {
      const word = off => pc98.read(p + off) | pc98.read(p + off + 1) << 8;
      return { fired: pc98.read(p + 8), result: pc98.read(p + 9), sp: word(10), entrySp: word(12),
        ip: word(14), cs: word(16), flags: word(18), segment: word(20), start: word(22), end: word(24) };
    }, state);
    expect(values.fired).toBe(1); expect(values.result).toBe(1);
    expect(values.entrySp).toBe(values.sp - 6);
    expect(values.ip).toBeGreaterThanOrEqual(values.start); expect(values.ip).toBeLessThan(values.end);
    expect(values.cs).toBe(values.segment); expect(values.flags & 0x200).toBe(0x200);
    expect(state).toBe(values.segment * 16 + 0x300);
    for (let i = 0; i < 10; i++) await page.evaluate(() => pc98.core.step(100000));
    expect(await page.evaluate(p => pc98.read(p + 8), state)).toBe(1);
    expect((await page.evaluate(() => pc98.stats())).error).toBe('');
  });
}
