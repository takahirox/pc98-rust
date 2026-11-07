import { test, expect, inspect, mediaGuestState } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { trackMemory } from './helpers/memory.js';

const manifest = JSON.parse(await fs.readFile('fixtures/desktop-manifest.json'));
const prefix = await fs.readFile('web/freedos/desktop.bin');
const data = await fs.readFile('fixtures/floppy_data/second.d88');
data[0x1a] = 1; // Same independent FAT data, explicitly write-protected D88.
const swapped = await fs.readFile('fixtures/floppy_data/swapped.hdm');
async function until(page, expected) {
  const state = await inspect(page, expected => {
    for (let slots=0; slots<8000000; slots+=10000) {
      if (pc98.text().includes(expected) || pc98.stats().error) break;
      pc98.advance(10000); pc98.core.audio_clear();
    }
    return {text: pc98.text(), stats: pc98.stats()};
  }, expected);
  expect(state.stats.error).toBe(''); expect(state.text).toContain(expected);
  return state;
}
async function command(page, text) {
  await page.locator('#screen').focus();
  for (const char of text+'\r') {
    const code = char===':' ? 'Quote' : char==='.' ? 'Period' : char===' ' ? 'Space' :
      char==='>' ? 'Period' : char==='\r' ? 'Enter' : `Key${char.toUpperCase()}`;
    if (char==='>') await page.keyboard.down('Shift');
    await page.keyboard.down(code);
    await inspect(page, () => { pc98.advance(50000); pc98.core.audio_clear(); });
    await page.keyboard.up(code);
    if (char==='>') await page.keyboard.up('Shift');
    await inspect(page, () => { pc98.advance(10000); pc98.core.audio_clear(); });
  }
}
async function checkMemory(page, high) {
  if (high) {
    await command(page, 'xmscheck');
    await until(page, 'XMSCHECK HMA/A20/move/invalid/recovery/free OK');
  }
  const paragraphs = high ? 38729 : 31116;
  const bytes = high ? 619664 : 497856;
  expect(paragraphs * 16).toBe(bytes);
  if (high) { expect(bytes).toBeGreaterThanOrEqual(552960); expect(bytes).toBeGreaterThan(497856); }
  // Each command returns its allocations. A second execution must give exactly
  // the same successful allocation, including after the XMS recovery probe.
  for (let repeat = 0; repeat < 2; repeat++) {
    await command(page, 'cls');
    await command(page, 'memsize');
    await until(page, `MEMSIZE paragraphs=${paragraphs} bytes=${bytes} allocated/freed/restored OK`);
    await until(page, 'A:\\>');
  }
}

async function ready(page) {
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({time}); await page.clock.pauseAt(time);
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
}

test('fresh installed desktop: exact kernel handoff, responsive DOS, live floppies and explicit boot/reset', async ({page, browser}, info) => {
  test.setTimeout(120000);
  const errors=[]; page.on('pageerror', e=>errors.push(e.message));
  const memory=await trackMemory(browser);
  const start=performance.now();
  try {
    await ready(page);
    const mountedMs=Math.round(performance.now()-start);
    expect(await inspect(page, () => [pc98.core.hdi_len(), pc98.core.export_disk(0), pc98.core.export_len(), pc98.core.export_disk(1), pc98.core.export_len()]))
      .toEqual([manifest.bytes,0,0,0,0]);
    const kernel=prefix.subarray(4096+manifest.fat.data_lba*512,4096+manifest.fat.data_lba*512+49938);
    for(let run=0;run<2;run++) {
      if(run) await page.locator('#reset').click();
      const handoff=await inspect(page, () => {
        for(let slots=0;slots<100000;slots++) {
          if(pc98.core.metric(17)===0x60 && pc98.core.metric(18)===0) return {
            bytes:Array.from({length:49938},(_,i)=>pc98.read(0x600+i)), reads:pc98.stats().hddReads};
          pc98.core.step(1);
        }
        return null;
      });
      expect(handoff).not.toBeNull(); expect(Buffer.from(handoff.bytes).equals(kernel)).toBe(true);
      expect(handoff.reads).toBeGreaterThan(100);
      const boot = await until(page,'A:\\>');
      expect(boot.text).toContain('NEC PC-98 version');
      expect(boot.text).toContain('19836 Bytes in HMA');
      await checkMemory(page, true);
      await command(page,'probe'); await until(page,'PC98 DOS PROBE OK');
      await command(page,'echo responsive'); await until(page,'responsive');
      expect(await inspect(page, () => pc98.stats().reads)).toBe(0);
    }
    await memory.sample();
    const measurements=await inspect(page, () => ({
      assets:performance.getEntriesByType('resource').map(r=>({name:new URL(r.name).pathname,bytes:r.encodedBodySize})),
      wasmBytes:pc98.core.memory.buffer.byteLength, residentBytes:pc98.core.hdi_resident_bytes(),
      jsHeapBytes:performance.memory?.usedJSHeapSize,
    }));
    measurements.mountedMs=mountedMs;
    measurements.steadyRss=memory.current();
    measurements.rendererPeakRssKiB=memory.peak(); measurements.browserPeakRssKiB=memory.totalPeak();
    expect(measurements.wasmBytes).toBeLessThan(32*1024*1024);
    expect(measurements.residentBytes).toBeLessThan(1024*1024);
    // Real Resource Timing is measured in the automatic Worker boot test below.
    delete measurements.assets;
    console.log(`${info.project.name} desktop startup ${JSON.stringify(measurements)}`);
    // Guest DOS assigns HDD A:, physical floppies B:/C:. Verify BIOS enumeration.
    expect(await inspect(page, () => [pc98.read(0x66c),pc98.read(0x66d),pc98.read(0x66e)]))
      .toEqual([0x80,0x90,0x91]);
    for (const [drive, letter] of [['0','b'],['1','c']]) {
      await page.locator('#drive').selectOption(drive);
      const before=await mediaGuestState(page);
      await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({name:'second.d88',mimeType:'application/octet-stream',buffer:data});
      await expect(page.locator('#media-name')).toHaveText('second.d88');
      expect(await mediaGuestState(page)).toEqual(before);
      await expect(page.locator('#boot')).toHaveValue('1');
      await command(page,letter+':'); await until(page,letter.toUpperCase()+':\\>');
      await command(page,'dir'); await until(page,'SECOND');
      const reads=await inspect(page, ()=>pc98.stats().reads);
      await command(page,'type second.txt'); await until(page,'ORIGINAL FDD2 FILE CONTENTS');
      expect(await inspect(page, ()=>pc98.stats().reads)).toBeGreaterThan(reads);
      expect(Buffer.from(await inspect(page, d=>pc98.exportDisk(d),Number(drive))).equals(data)).toBe(true);
      await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({name:'swapped.hdm',mimeType:'application/octet-stream',buffer:swapped});
      await expect(page.locator('#media-name')).toHaveText('swapped.hdm');
      await command(page,'cls'); await command(page,'dir');
      expect((await until(page,'SWAPPED')).text).not.toContain('SECOND');
      await command(page,'type swapped.txt'); await until(page,'REPLACEMENT FDD2 FILE CONTENTS');
      expect(Buffer.from(await inspect(page, d=>pc98.exportDisk(d),Number(drive))).equals(swapped)).toBe(true);
      await page.locator('#eject').click();
      await command(page,'cls'); await command(page,'dir'); await until(page,'(R)');
      await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({name:'second.d88',mimeType:'application/octet-stream',buffer:data});
      await expect(page.locator('#media-name')).toHaveText('second.d88');
      await command(page,'r'); // Reinsert, then retry through the real DOS error handler.
      await until(page,letter.toUpperCase()+':\\>');
      await command(page,'a:');
    }
    expect(await inspect(page, ()=>pc98.stats().writes)).toBe(0);
    await page.locator('#drive').selectOption('0'); await page.locator('#hello').click();
    // The bundled image is fetched asynchronously. Wait for media loading and
    // reset to finish before inspecting the Worker-owned Wasm core.
    await expect(page.locator('#media-name')).toHaveText('hello98.tfd');
    await expect(page.locator('#reset')).toBeEnabled();
    await until(page,'Hello world!!');
    await page.locator('#boot').selectOption('1'); await page.locator('#reset').click();
    await until(page,'A:\\>'); await command(page,'echo back'); await until(page,'back');
    expect(errors).toEqual([]);
  } finally { await memory.stop(); }
});

test('default desktop boots in the Worker without user action and reset reaches an interactive prompt', async ({page}, info) => {
  const start=performance.now(); await page.goto('./');
  await expect.poll(()=>page.evaluate(()=>window.pc98?.text()),{timeout:20000}).toContain('A:\\>');
  const coldBootMs=Math.round(performance.now()-start);
  const assets=await page.evaluate(()=>[...performance.getEntriesByType('navigation'),...performance.getEntriesByType('resource')].map(r=>({name:new URL(r.name).pathname,bytes:r.encodedBodySize,transferBytes:r.transferSize})));
  const totalBodyBytes=assets.reduce((sum,a)=>sum+a.bytes,0);
  expect(totalBodyBytes).toBeLessThan(2*1024*1024);
  expect(assets.find(a=>a.name.endsWith('/desktop.bin')).bytes).toBe(manifest.prefix_bytes);
  expect(assets.some(a=>a.name.includes('source.tar'))).toBe(false);
  console.log(`${info.project.name} actual startup downloads ${JSON.stringify(assets)}`);
  // Physical events with actual frame scheduling, no manual core stepping.
  await page.locator('#screen').focus(); await page.keyboard.type('echo alive',{delay:70}); await page.keyboard.press('Enter');
  await expect.poll(()=>inspect(page, ()=>pc98.text())).toContain('\nalive');
  await page.locator('#reset').click();
  await expect.poll(()=>inspect(page, ()=>pc98.text()),{timeout:20000}).toContain('A:\\>');
  console.log(`${info.project.name} automatic cold boot ${coldBootMs} ms`);
});

test('dirty desktop: cancel replacement/eject/restore, export guest writes, reimport and profile retention', async ({page},info) => {
  test.setTimeout(120000);
  await ready(page); await until(page,'A:\\>');
  await command(page,'echo saved>saved.txt'); await command(page,'type saved.txt'); await until(page,'saved');
  expect(await inspect(page, ()=>(pc98.core.hdi_dirty_page(0) >>> 0))).not.toBe(0xffffffff);
  const dialogs=[];
  page.on('dialog',async d=>{dialogs.push(d.message());await d.dismiss();});
  await page.locator('#eject').click(); await expect.poll(()=>dialogs.length).toBe(1);
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#default-hdd').click(); await expect.poll(()=>dialogs.length).toBe(2);
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles('fixtures/freedos/shell.hdi');
  await expect.poll(()=>dialogs.length).toBe(3);
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#media-name')).toHaveText('FreeDOS-128MiB.hdi');
  expect(dialogs).toHaveLength(3); expect(dialogs.every(t=>t.includes('guest writes'))).toBe(true);
  const downloadEvent=page.waitForEvent('download'); await page.locator('#export').click();
  const download=await downloadEvent; expect(await download.failure()).toBeNull();
  const file=info.outputPath('saved.hdi'); await download.saveAs(file);
  expect((await fs.stat(file)).size).toBe(manifest.bytes);
  // All logical bytes export, and DOS changes make the saved disk differ.
  const bytes=await fs.readFile(file); expect(createHash('sha256').update(bytes).digest('hex')).not.toBe(manifest.image_sha256);
  page.removeAllListeners('dialog'); page.on('dialog',d=>d.accept());
  await page.locator('#eject').click(); await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles(file);
  await expect(page.locator('#media-name')).toHaveText('saved.hdi');
  await until(page,'A:\\>'); await command(page,'type saved.txt'); await until(page,'\nsaved');
  await page.locator('#profile').selectOption('0');
  expect(await inspect(page, ()=>pc98.read(0x584))).toBe(0x90);
  await expect(page.locator('#boot')).toBeDisabled(); await expect(page.locator('#default-hdd')).toBeDisabled();
  await page.locator('#profile').selectOption('1'); await until(page,'A:\\>');
  await command(page,'type saved.txt'); await until(page,'\nsaved');
  await page.locator('#default-hdd').click();
  await expect(page.locator('#media-name')).toHaveText('FreeDOS-128MiB.hdi');
  await until(page,'A:\\>'); expect(await inspect(page, ()=>(pc98.core.hdi_dirty_page(0) >>> 0))).toBe(0xffffffff);
  await checkMemory(page, true);
});

for(const [kind, size] of [
  ['missing', undefined], ['string', String(manifest.prefix_bytes)],
  ['array', [manifest.prefix_bytes]], ['fractional', manifest.prefix_bytes + 0.5],
]) {
  test(`desktop rejects ${kind} prefix size before requesting the template`,async({page})=>{
    let templateRequests=0;
    page.on('request',request=>{
      if(new URL(request.url()).pathname.endsWith('/freedos/desktop.bin')) templateRequests++;
    });
    await page.route('**/freedos/manifest.json',route=>route.fulfill({json:{...manifest,prefix_bytes:size}}));
    await ready(page);
    await expect(page.locator('#error')).toContainText('Invalid FreeDOS template geometry/size');
    expect(templateRequests).toBe(0);
    expect(await inspect(page, ()=>pc98.core.hdi_len())).toBe(0);
    await expect(page.locator('#default-hdd')).toBeEnabled();
  });
}

for(const failure of ['fetch','truncated','checksum','geometry','allocation']) {
  test(`desktop ${failure} failure is recoverable and cannot publish a partial mount`,async({page})=>{
    if(failure==='allocation') {
      await page.addInitScript(()=>{
        const Original=Uint8Array;
        globalThis.Uint8Array=new Proxy(Original,{construct(target,args){
          if(args[0]===1024*1024) { globalThis.Uint8Array=Original; throw Error('Simulated memory allocation failure'); }
          return Reflect.construct(target,args);
        }});
      });
    } else if(failure==='geometry') {
      await page.route('**/freedos/manifest.json',async route=>route.fulfill({json:{...manifest,bytes:123}}));
    } else {
      await page.route('**/freedos/desktop.bin',route=>failure==='fetch' ? route.fulfill({status:503,body:'Unavailable'}) :
        route.fulfill({body:failure==='truncated' ? prefix.subarray(0,100) : Buffer.alloc(prefix.length)}));
    }
    await ready(page); await expect(page.locator('#error')).toBeVisible();
    expect(await inspect(page, ()=>pc98.core.hdi_len())).toBe(0);
    await page.unrouteAll(); await page.locator('#default-hdd').click();
    await expect(page.locator('#media-name')).toHaveText('FreeDOS-128MiB.hdi');
    await until(page,'A:\\>');
  });
}


test('startup locks profile/media changes and ignores a competing drop until assembly completes',async({page})=>{
  let release;
  const pending=new Promise(resolve=>{release=resolve;});
  await page.route('**/freedos/desktop.bin',async route=>{await pending;await route.continue();});
  await page.goto('./');
  await expect(page.locator('#status')).toContainText('Loading FreeDOS HDD');
  for(const id of ['profile','boot','file','drive','game','reset']) await expect(page.locator('#'+id)).toBeDisabled();
  await page.locator('#drop').evaluate(drop=>{
    const transfer=new DataTransfer();transfer.items.add(new File([new Uint8Array(1261568)],'competing.hdm'));
    drop.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:transfer}));
  });
  release(); await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#media-name')).toHaveText('FreeDOS-128MiB.hdi');
  expect(await inspect(page, ()=>{pc98.core.export_disk(0);return pc98.core.export_len();})).toBe(0);
});


test('published license page provides accessible checksum-matching corresponding sources and notices',async({page})=>{
  await page.goto('./licenses.html');
  const hashes=JSON.parse(await fs.readFile('fixtures/desktop-publication.json'));
  for(const name of ['kernel-source.tar.gz','freecom-source.tar.gz','desktop-source.tar','COPYING','FreeCOM-LICENSE','NOTICE.txt','manifest.json',
    'himemx-release.zip','himemx-source.tar.gz','HimemX-LICENSE.txt',
    'HimemX-README.txt','HimemX-HimemX98.txt']) {
    const href=await page.locator(`a[href="freedos/${name}"]`).getAttribute('href');
    const response=await page.request.get(new URL(href,page.url()).href);
    expect(response.ok()).toBe(true);
    expect(createHash('sha256').update(await response.body()).digest('hex')).toBe(hashes[name]);
  }
});


test('DOS LOW baseline: same Ap desktop and guest allocator, cold and reset', async ({page}) => {
  const low = Buffer.from(prefix);
  let start, size;
  const root = 4096 + manifest.fat.root_lba * 512;
  for (let at = root; at < root + 512 * 32; at += 32) {
    if (low.subarray(at, at + 11).toString() !== 'FDCONFIGSYS') continue;
    start = 4096 + manifest.fat.data_lba * 512 + (low.readUInt16LE(at + 26) - 2) * 4096;
    size = low.readUInt32LE(at + 28);
    break;
  }
  expect(start).toBeDefined();
  low.fill(0x1a, start, start + size);
  low.write('DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n', start, 'ascii');
  const digest = createHash('sha256').update(low).digest('hex');
  await page.route('**/freedos/manifest.json', route => route.fulfill({json: {...manifest, prefix_sha256: digest}}));
  await page.route('**/freedos/desktop.bin', route => route.fulfill({body: low}));
  await ready(page);
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    const boot = await until(page, 'A:\\>');
    expect(boot.text).not.toContain('HimemX 3.34');
    expect(boot.text).not.toContain('Bytes in HMA');
    await checkMemory(page, false);
  }
});
