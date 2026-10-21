import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
const BASE = 0x21400;
async function ready(page) {
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('./');
  await expect(page.locator('#game')).toBeEnabled();
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  return errors;
}
async function value(page, offset, expected) {
  await expect.poll(() => page.evaluate((addr) => pc98.read(addr), BASE + offset)).toBe(expected);
}
async function press(page, code, moves) {
  await page.locator('#screen').focus();
  await page.keyboard.press(code);
  await expect.poll(() => page.evaluate((addr) => pc98.read(addr) | pc98.read(addr+1)<<8, BASE+12)).toBe(moves);
}

test('actual Wasm guest gameplay, PCM audio, pause, save export and import work offline', async ({ page }) => {
  const errors = await ready(page);
  await page.locator('#game').click();
  await value(page, 8, 1);
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('STAR TRAIL');
  // Once the local assets load, any network call is a failure for this test.
  const unexpected = [];
  await page.route('**/*', route => { unexpected.push(route.request().url()); return route.abort(); });
  const before = await page.locator('#screen').screenshot();
  let moves=0;
  for(let i=0;i<3;i++) await press(page,'ArrowRight',++moves);
  await value(page,10,1);
  const after = await page.locator('#screen').screenshot();
  expect(Buffer.compare(before,after)).not.toBe(0);
  for(let i=0;i<2;i++) await press(page,'ArrowDown',++moves);
  for(let i=0;i<4;i++) await press(page,'ArrowRight',++moves);
  for(let i=0;i<3;i++) await press(page,'ArrowDown',++moves);
  for(let i=0;i<2;i++) await press(page,'ArrowRight',++moves);
  for(let i=0;i<3;i++) await press(page,'ArrowUp',++moves);
  await value(page,10,3); await value(page,14,1);
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('YOU WIN!');
  await expect.poll(() => page.evaluate(() => pc98.stats().writes)).toBeGreaterThanOrEqual(17);
  await expect.poll(() => page.evaluate(() => pc98.stats().audioEnergy)).toBeGreaterThan(1);
  const stats = await page.evaluate(() => pc98.stats());
  expect(stats.audioState).toBe('running'); expect(stats.scheduledSamples).toBeGreaterThan(1000);
  expect(stats.instructions).toBeGreaterThan(50_000); expect(stats.reads).toBe(8); expect(stats.error).toBe('');
  await page.locator('#pause').click();
  const paused=await page.evaluate(()=>pc98.stats().instructions);
  await page.waitForTimeout(100);
  expect(await page.evaluate(()=>pc98.stats().instructions)).toBe(paused);
  const downloaded=page.waitForEvent('download'); await page.locator('#export').click();
  const file=await downloaded; const disk=await fs.readFile(await file.path());
  expect(disk.length).toBe(1261568); expect(disk[7*1024+10]).toBe(3);
  expect(disk.subarray(7*1024,7*1024+8).toString()).toBe('STAR98SV');
  await page.locator('#file').setInputFiles({name:'saved.hdm',mimeType:'application/octet-stream',buffer:disk});
  await value(page,8,1); await value(page,10,0);
  await expect.poll(()=>page.evaluate(()=>pc98.read(0x2180a))).toBe(3);
  await expect(page.locator('#media-name')).toHaveText('saved.hdm');
  await page.locator('#reset').click(); await value(page,8,1); await value(page,10,0);
  await expect(page.locator('#error')).toBeHidden();
  expect(errors).toEqual([]);expect(unexpected).toEqual([]);
});

test('independent hello disk boots through FDC and guest BEEP reaches Web Audio', async ({ page }) => {
  const errors=await ready(page);await page.locator('#hello').click();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
  await expect.poll(()=>page.evaluate(()=>pc98.stats().audioEnergy)).toBeGreaterThan(1);
  const state=await page.evaluate(()=>{
    const canvas=document.querySelector('#screen');const pixels=canvas.getContext('2d').getImageData(0,0,640,16).data;
    let white=0;for(let i=0;i<pixels.length;i+=4)if(pixels[i]===255&&pixels[i+1]===255&&pixels[i+2]===255)white++;
    return {...pc98.stats(),white};
  });
  expect(state.reads).toBe(1);expect(state.beep).toBe(97);expect(state.beepEdges).toBe(1);expect(state.white).toBeGreaterThan(100);
  expect(state.error).toBe('');expect(errors).toEqual([]);
});

test('floppy validation, second drive, eject, and virtual PC-98 keyboard', async ({ page })=>{
  await ready(page);await page.locator('#game').click();await value(page,8,1);
  await page.locator('#file').setInputFiles({name:'corrupt.d88',mimeType:'application/octet-stream',buffer:Buffer.alloc(100)});
  await expect(page.locator('#error')).toContainText('truncated D88');
  await expect(page.locator('#media-name')).toHaveText('startrail.hdm');
  await page.locator('#drive').selectOption('1');
  await page.locator('#file').setInputFiles({name:'blank.hdm',mimeType:'application/octet-stream',buffer:Buffer.alloc(1261568)});
  await expect(page.locator('#media-name')).toHaveText('blank.hdm');
  await page.locator('#eject').click();await expect(page.locator('#media-name')).toHaveText('No disk');await expect(page.locator('#export')).toBeDisabled();
  await page.locator('summary').click();const right=page.locator('[data-scan="60"]');
  await right.click();await value(page,8,2);
});

test('missing or broken Wasm fails visibly and cannot masquerade as a working demo',async({page})=>{
  await page.route('**/core.wasm',route=>route.fulfill({status:404,body:'missing'}));
  await page.goto('./');await expect(page.locator('#error')).toContainText('Missing Wasm bundle');
  await expect(page.locator('#game')).toBeDisabled();expect(await page.evaluate(()=>window.pc98)).toBeUndefined();
  await page.route('**/core.wasm',route=>route.fulfill({status:200,body:'this is not WebAssembly'}));
  await page.reload();await expect(page.locator('#status')).toHaveText('Unable to start');await expect(page.locator('#game')).toBeDisabled();
});
