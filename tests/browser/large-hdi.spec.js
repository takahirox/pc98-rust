import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { trackMemory } from './helpers/memory.js';

const probe = await fs.readFile(new URL('../../fixtures/large_hdi/probe.bin', import.meta.url));
const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/large-hdi-manifest.json', import.meta.url)));
expect(createHash('sha256').update(probe).digest('hex')).toBe(manifest.binary_sha256);
const chunkBytes = 1024 * 1024;
async function makeImage(file, cylinders, dense = false) {
  const size = cylinders * 16 * 32 * 512, header = Buffer.alloc(4096);
  [4096, size, 512, 32, 16, cylinders].forEach((v,i) => header.writeUInt32LE(v, 8+i*4));
  const handle = await fs.open(file, 'w');
  try {
    await handle.truncate(size + header.length);
    if (dense) {
      // Representative occupied media: 510 MiB nonzero, generated in one
      // reusable MiB buffer. Leave IPL and final probe sectors untouched.
      const block = Buffer.alloc(chunkBytes, 0x6c);
      for (let at = chunkBytes; at < size - chunkBytes; at += chunkBytes) {
        await handle.write(block, 0, block.length, at);
      }
    }
    await handle.write(header, 0, header.length, 0);
    await handle.write(probe, 0, probe.length, header.length);
    // Nonzero unchanged bytes near the tail ensure export preserves actual
    // original data, not just zero-fill. Outside all guest-written sectors.
    await handle.write(Buffer.from('UNCHANGED HIGH HDI BYTES'), 0, 24, size - 4096);
  } finally { await handle.close(); }
}
async function digest(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}
async function expectedDigest(file, cylinders) {
  const handle = await fs.open(file, 'r');
  const hash = createHash('sha256'), size = cylinders * 16 * 32 * 512 + 4096;
  const lbas = [119, 65535, cylinders * 256 - 1, cylinders * 512 - 2];
  try {
    for (let at = 0; at < size; at += chunkBytes) {
      const block = Buffer.alloc(Math.min(chunkBytes, size-at));
      expect((await handle.read(block, 0, block.length, at)).bytesRead).toBe(block.length);
      for (const lba of lbas) {
        const start = 4096+lba*512, end = start+1024;
        const from = Math.max(start,at), to = Math.min(end,at+block.length);
        if (from < to) block.fill(0x5a, from-at, to-at);
      }
      hash.update(block);
    }
  } finally { await handle.close(); }
  return hash.digest('hex');
}
async function runGuest(page, persisted, maxResident = 6*65536) {
  const result = await inspect(page, () => {
    for (let slots=0;slots<500000;slots+=1000) {
      pc98.advance(1000); pc98.core.audio_clear();
      const text=pc98.text(), error=pc98.stats().error;
      if (text.includes('LARGE HDI PASS') || text.includes('FAIL') || error) return {text,error,slots};
    }
    return {text:pc98.text(),error:pc98.stats().error};
  });
  expect(result.error).toBe('');
  expect(result.text).toContain('LARGE HDI PASS');
  expect(result.text.includes('LARGE HDI PERSIST')).toBe(persisted);
  expect(await inspect(page, () => pc98.stats().hddWrites)).toBe(9);
  expect(await inspect(page, () => pc98.core.hdi_resident_bytes())).toBeLessThanOrEqual(maxResident);
}

test('512 MiB local HDI: real guest high I/O, reset, full streamed export/reimport and bounded memory', async ({ page, browser }, testInfo) => {
  test.setTimeout(120000);
  const folder = await fs.mkdtemp(path.join(os.tmpdir(), 'pc98-large-'));
  const input=path.join(folder,'large.hdi'), output=path.join(folder,'exported.hdi');
  const errors=[]; page.on('pageerror',e=>errors.push(e.message));
  const memory=await trackMemory(browser);
  const timings={};
  try {
    await makeImage(input,2048);
    page.on('dialog', dialog => dialog.accept());
    const now=new Date('2026-01-01T00:00:00Z');
    await page.clock.install({time:now}); await page.clock.pauseAt(now);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await memory.sample();
    timings.baselineRendererRssKiB=memory.peak();
    timings.baselineBrowserRssKiB=memory.totalPeak();
    memory.reset();
    let start=performance.now();
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(input);
    await expect(page.locator('#media-name')).toHaveText('large.hdi', {timeout:60000});
    timings.importMs=Math.round(performance.now()-start);
    await memory.sample(); timings.importPeakRendererRssKiB=memory.peak();
    timings.importPeakBrowserRssKiB=memory.totalPeak();
    await runGuest(page,false);
    await page.locator('#reset').click();
    await runGuest(page,true);
    timings.wasmHighWaterBytes=await inspect(page, ()=>pc98.core.memory.buffer.byteLength);
    expect(timings.wasmHighWaterBytes).toBeLessThan(32*1024*1024);
    // A failed replacement and incomplete staging import cannot lose prior writes.
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({name:'bad.hdi',mimeType:'application/octet-stream',buffer:Buffer.alloc(32)});
    await expect(page.locator('#error')).toContainText('invalid HDI geometry');
    await expect(page.locator('#media-name')).toHaveText('large.hdi');
    await page.locator('#reset').click(); await runGuest(page,true);
    await page.evaluate(() => {
      const original=Blob.prototype.arrayBuffer;
      let calls=0;
      Blob.prototype.arrayBuffer=function() {
        if (++calls===3) {
          Blob.prototype.arrayBuffer=original;
          return Promise.reject(new Error('Local HDI read failed during import'));
        }
        return original.call(this);
      };
    });
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(input);
    await expect(page.locator('#error')).toContainText('Local HDI read failed during import');
    await expect(page.locator('#media-name')).toHaveText('large.hdi');
    await page.locator('#reset').click(); await runGuest(page,true);
    // A resource failure must be visible and recoverable without losing the
    // guest's writes or leaving controls disabled.
    await page.evaluate(() => {
      const OriginalBlob=Blob;
      globalThis.Blob=new Proxy(OriginalBlob, {
        construct() {
          globalThis.Blob=OriginalBlob;
          throw new Error('Export snapshot allocation failed');
        },
      });
    });
    await page.locator('#export').click();
    await expect(page.locator('#error')).toContainText('Cannot export HDI: Export snapshot allocation failed');
    await expect(page.locator('#error')).toContainText('Disk remains mounted');
    await expect(page.locator('#reset')).toBeEnabled();
    expect(await inspect(page, () => pc98.stats().running)).toBe(true);
    const expected=await expectedDigest(input,2048);
    memory.reset();
    start=performance.now();
    const downloadEvent=page.waitForEvent('download');
    await page.locator('#export').click();
    const download=await downloadEvent;
    // Saving over the selected file invalidates Chromium's original File
    // snapshot. Subsequent exports must use the mounted bytes, including all
    // untouched sectors, rather than that now-unreadable snapshot.
    await download.saveAs(input);
    expect(await download.failure()).toBeNull();
    timings.exportDownloadMs=Math.round(performance.now()-start);
    await memory.sample(); timings.exportPeakRendererRssKiB=memory.peak();
    timings.exportPeakBrowserRssKiB=memory.totalPeak();
    expect((await fs.stat(input)).size).toBe(512*1024*1024+4096);
    expect(await digest(input)).toBe(expected);
    start=performance.now();
    const againEvent=page.waitForEvent('download');
    await page.locator('#export').click();
    const again=await againEvent;
    await again.saveAs(output);
    expect(await again.failure()).toBeNull();
    timings.repeatExportDownloadMs=Math.round(performance.now()-start);
    expect((await fs.stat(output)).size).toBe(512*1024*1024+4096);
    expect(await digest(output)).toBe(expected);
    await page.locator('#eject').click();
    await expect(page.locator('#export')).toBeDisabled();
    memory.reset(); start=performance.now();
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(output);
    await expect(page.locator('#media-name')).toHaveText('exported.hdi',{timeout:60000});
    timings.reimportMs=Math.round(performance.now()-start);
    await runGuest(page,true);
    await memory.sample();
    timings.reimportPeakRendererRssKiB=memory.peak();
    timings.reimportPeakBrowserRssKiB=memory.totalPeak();
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
    console.log(`${testInfo.project.name} large HDI measurements ${JSON.stringify(timings)}`);
  } finally {
    await memory.stop();
    await fs.rm(folder,{recursive:true,force:true});
  }
});

test('1 GiB local HDI: actual Ap high-sector guest I/O', async ({ page },testInfo) => {
  test.skip(testInfo.project.name !== 'local','Desired 1 GiB capacity sampled once; 512 MiB full round-trip covers both roots.');
  test.setTimeout(60000);
  const folder=await fs.mkdtemp(path.join(os.tmpdir(),'pc98-gib-'));
  try {
    const input=path.join(folder,'one-gib.hdi'); await makeImage(input,4096);
    page.on('dialog', dialog => dialog.accept());
    const now=new Date('2026-01-01T00:00:00Z');
    await page.clock.install({time:now}); await page.clock.pauseAt(now);
    await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
    const start=performance.now();
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(input);
    await expect(page.locator('#media-name')).toHaveText('one-gib.hdi',{timeout:45000});
    await runGuest(page,false);
    console.log(`1 GiB import and guest probe ${Math.round(performance.now()-start)} ms`);
  } finally { await fs.rm(folder,{recursive:true,force:true}); }
});

test('512 MiB FAT16: unmodified FreeDOS shell reads and writes files above 128 MiB', async ({ page },testInfo) => {
  test.skip(testInfo.project.name !== 'local','DOS integration sampled once; capacity/media round-trip covers both roots.');
  test.setTimeout(120000);
  page.on('dialog', dialog => dialog.accept());
  const now=new Date('2026-01-01T00:00:00Z');
  await page.clock.install({time:now}); await page.clock.pauseAt(now);
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(path.resolve('fixtures/freedos/large-fat16.hdi'));
  await expect(page.locator('#media-name')).toHaveText('large-fat16.hdi',{timeout:60000});
  const folder=await fs.mkdtemp(path.join(os.tmpdir(),'pc98-fat16-'));
  try {
    for (let run=0;run<3;run++) {
      if (run===1) await page.locator('#reset').click();
      if (run===2) {
        const output=path.join(folder,'dos-saved.hdi');
        const downloading=page.waitForEvent('download'); await page.locator('#export').click();
        const download=await downloading; await download.saveAs(output);
        expect(await download.failure()).toBeNull();
        expect((await fs.stat(output)).size).toBe(512*1024*1024+4096);
        await page.locator('#eject').click();
        await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(output);
        await expect(page.locator('#media-name')).toHaveText('dos-saved.hdi',{timeout:60000});
      }
      const result=await inspect(page, ()=>{
        for(let slots=0;slots<8000000;slots+=10000) {
          pc98.advance(10000); pc98.core.audio_clear();
          const text=pc98.text(),error=pc98.stats().error;
          if(error || (text.includes('LARGE DOS READY') && text.includes('A:\\>'))) return {text,error,slots};
        }
        return {text:pc98.text(),error:pc98.stats().error};
      });
      expect(result.error).toBe('');
      expect(result.text).toContain('HIGH FAT16 SOURCE');
      expect(result.text.split('LARGE DOS WRITE').length-1).toBe(run===0?1:2);
      expect(result.text).toContain('LARGE DOS READY');
      expect(result.text).toContain('A:\\>');
      const file=await inspect(page, ()=>{
        const rootAt=4096+(512+258)*512;
        const ptr=pc98.core.hdi_read_chunk(rootAt,16384);
        const root=new Uint8Array(pc98.core.memory.buffer,ptr,16384);
        let cluster;
        for(let at=0;at<root.length;at+=32) {
          if(new TextDecoder().decode(root.subarray(at,at+11))==='NEW     TXT') {
            cluster=root[at+26]|root[at+27]<<8; break;
          }
        }
        if(!cluster) return {};
        const lba=802+(cluster-2)*32;
        const data=pc98.core.hdi_read_chunk(4096+lba*512,512);
        return {cluster,lba,text:new TextDecoder().decode(new Uint8Array(pc98.core.memory.buffer,data,17))};
      });
      expect(file.cluster).toBeGreaterThanOrEqual(8193);
      expect(file.lba*512).toBeGreaterThan(128*1024*1024);
      expect(file.text).toBe('LARGE DOS WRITE\r\n');
      // Actual DOS application and input handling, not just AUTOEXEC output.
      await inspect(page, ()=>{
        for(const scan of [0x19,0x13,0x18,0x2d,0x12,0x1c]) {
          pc98.core.key(scan,1); pc98.advance(50000);
          pc98.core.key(scan,0); pc98.advance(10000);
        }
        pc98.advance(100000); pc98.core.audio_clear();
      });
      expect(await inspect(page, ()=>pc98.text())).toContain('PC98 DOS PROBE OK');
      expect(await inspect(page, ()=>pc98.stats().error)).toBe('');
      console.log(`Large FAT16 DOS boot ${run}: ${result.slots} slots; high-file cluster ${file.cluster}`);
    }
  } finally { await fs.rm(folder,{recursive:true,force:true}); }
});

test('occupied 512 MiB HDI imports once and exports without a second Wasm image', async ({ page, browser },testInfo) => {
  test.skip(testInfo.project.name !== 'local','Occupied-media resource scenario sampled once.');
  test.setTimeout(120000);
  const folder=await fs.mkdtemp(path.join(os.tmpdir(),'pc98-dense-'));
  const memory=await trackMemory(browser);
  try {
    const input=path.join(folder,'occupied.hdi'), output=path.join(folder,'saved.hdi');
    await makeImage(input,2048,true);
    page.on('dialog', dialog => dialog.accept());
    const now=new Date('2026-01-01T00:00:00Z');
    await page.clock.install({time:now}); await page.clock.pauseAt(now);
    await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
    let start=performance.now();
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(input);
    await expect(page.locator('#media-name')).toHaveText('occupied.hdi',{timeout:60000});
    const importMs=Math.round(performance.now()-start);
    await memory.sample(); const importPeakRendererRssKiB=memory.peak(), importPeakBrowserRssKiB=memory.totalPeak();
    await runGuest(page,false,512*1024*1024+4096);
    const residentBytes=await inspect(page, ()=>pc98.core.hdi_resident_bytes());
    expect(residentBytes).toBeGreaterThan(510*1024*1024);
    const wasmBytes=await inspect(page, ()=>pc98.core.memory.buffer.byteLength);
    expect(wasmBytes).toBeLessThan(600*1024*1024);
    start=performance.now();
    memory.reset();
    const downloading=page.waitForEvent('download'); await page.locator('#export').click();
    const download=await downloading; await download.saveAs(output);
    expect(await download.failure()).toBeNull();
    const exportMs=Math.round(performance.now()-start);
    await memory.sample(); const exportPeakRendererRssKiB=memory.peak(), exportPeakBrowserRssKiB=memory.totalPeak();
    expect(await inspect(page, ()=>pc98.core.memory.buffer.byteLength)).toBe(wasmBytes);
    expect(await digest(output)).toBe(await expectedDigest(input,2048));
    console.log(`Occupied 512 MiB HDI ${JSON.stringify({importMs,exportMs,residentBytes,wasmBytes,importPeakRendererRssKiB,exportPeakRendererRssKiB,importPeakBrowserRssKiB,exportPeakBrowserRssKiB})}`);
  } finally { await memory.stop(); await fs.rm(folder,{recursive:true,force:true}); }
});
