import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

const pattern = Array.from({ length: 32 }, (_, i) => i % 2 === 0
  ? 0x80 >> (Math.floor(i / 2) % 8) : (i * 7) ^ 0x53);
const ank = { A: [14,17,17,31,17,17,17], B: [30,17,17,30,17,17,30] };
async function stage(page, expected) {
  const snapshot = await page.evaluate(expected => {
    let slots = 0;
    while (pc98.read(0x8001) !== expected && slots < 200000) {
      slots += pc98.core.step(1000);
      pc98.core.audio_clear();
      if (pc98.stats().error || pc98.read(0x8001) === 255) break;
    }
    return { stage: pc98.read(0x8001), error: pc98.stats().error, text: pc98.text() };
  }, expected);
  expect(snapshot.error).toBe('');
  expect(snapshot.stage, snapshot.text).toBe(expected);
}
async function enter(page) {
  await page.locator('#screen').focus();
  await page.keyboard.press('Enter');
}
async function pixels(page, left, top, width, height) {
  return page.evaluate(({ left, top, width, height }) => {
    const ptr = pc98.core.frame();
    const frame = new Uint8Array(pc98.core.memory.buffer, ptr, 640 * 400 * 4);
    return Array.from({ length: height }, (_, y) => Array.from({ length: width }, (_, x) => {
      const i = ((top + y) * 640 + left + x) * 4;
      return Array.from(frame.subarray(i, i + 4));
    }));
  }, { left, top, width, height });
}
function rgba(ink) { return ink ? [255,255,255,255] : [0,0,0,255]; }
async function assertGlyph(page, stage) {
  const actual = await pixels(page, 0, 0, 16, stage === 4 ? 20 : 16);
  const expected = actual.map((row, y) => row.map((_, x) => {
    if (stage === 2 || y >= 16) return rgba(false);
    const byte = stage === 4 ? pattern[y * 2] : pattern[y * 2 + (x >> 3)];
    return rgba(Boolean(byte & (128 >> (stage === 4 ? x >> 1 : x & 7))));
  }));
  expect(actual).toEqual(expected);
}
async function assertAreasAndCursor(page) {
  for (const [ch, top] of [['A',16],['B',32]]) {
    const actual = await pixels(page,0,top,8,16);
    expect(actual).toEqual(Array.from({length:16},(_,y) => Array.from({length:8},(_,x) =>
      rgba(y > 0 && y < 15 && Boolean((ank[ch][(y-1)>>1] << 2) & (128 >> x))))));
  }
  const cursor = await pixels(page,32,0,8,16);
  expect(cursor).toEqual(Array.from({length:16},(_,y) => Array.from({length:8},() => rgba(y >= 14))));
}

test('Ap CRT family: original guest, KCG/areas/pitch/cursor pixels, timed blink and reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', msg => { if (msg.type() === 'error') errors.push(msg.text()); });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({time});
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/crt/crt.hdm',import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/crt-manifest.json',import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await page.locator('#file').setInputFiles({name:'crt.hdm',mimeType:'application/octet-stream',buffer:image});
  await page.evaluate(() => pc98.core.set_boot_source(0));
  for (let run = 0; run < 2; run++) {
    await page.locator('#reset').click();
    expect(await page.evaluate(() => [pc98.read(0x8001),pc98.read(0x53c),pc98.read(0xa4001)])).toEqual([0,0,255]);
    await stage(page,1);
    await assertGlyph(page,1);
    await assertAreasAndCursor(page);
    await enter(page);
    await stage(page,2);
    await assertGlyph(page,2);
    await assertAreasAndCursor(page);
    await enter(page);
    await stage(page,3);
    await assertGlyph(page,3);
    const samples = await page.evaluate(() => {
      const samples = [];
      for (let i=0;i<40;i++) {
        const clocks = pc98.core.metric(5);
        const ptr = pc98.core.frame();
        const frame = new Uint8Array(pc98.core.memory.buffer,ptr,640*400*4);
        const offset = (14*640+32)*4;
        samples.push({clocks,pixel:Array.from(frame.subarray(offset,offset+4))});
        pc98.core.step(5000);
        pc98.core.audio_clear();
      }
      return samples;
    });
    const states = new Set();
    for (const {clocks,pixel} of samples) {
      const on = Math.floor(Math.floor(clocks/133333)/4)%2 === 0;
      states.add(on);
      expect(pixel).toEqual(rgba(on));
    }
    expect(states.size).toBe(2);
    await enter(page);
    await stage(page,4);
    await assertGlyph(page,4);
    expect(await page.evaluate(() => pc98.read(0x53c))).toBe(3);
    await enter(page);
    await stage(page,0x5a);
    const result = await page.evaluate(() => Array.from({length:128},(_,i) => pc98.read(0x8000+i)));
    expect(result).toEqual([...Buffer.from(manifest.result_hex,'hex')]);
    expect(result.slice(34,66)).toEqual(pattern);
    expect(result.slice(68,100)).toEqual(pattern);
    for (const i of [0,2,3,5,6,7,8,9,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,66,67,100,127]) expect(result[i]).toBe(0xa5);
    expect(await page.evaluate(() => pc98.read(0x53c))).toBe(0);
    expect(await page.evaluate(() => pc98.text())).toContain('CRT FAMILY PASS');
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
  }
});
