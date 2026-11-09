// Opt-in local measurement; the installed game is never copied into web/.
// See docs/doukyuusei2-performance.md for installation and interpretation.
import {chromium} from '@playwright/test';
import {spawn} from 'node:child_process';
import {readFile, realpath, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import os from 'node:os';

const options = Object.fromEntries(process.argv.slice(2).map(value => value.replace(/^--/, '').split('=')));
const repeat = Number(options.repeat ?? 3), playback = options.playback === '1';
if (!Number.isInteger(repeat) || repeat < 1 || repeat > 10) throw Error('Expected --repeat=1..10');
if (options.channel && options.channel !== 'chrome') throw Error('Use --channel=chrome for installed Google Chrome, or omit it for pinned Chromium');
const root = await realpath('.');
for (const name of ['media', 'output']) {
  if (!options[name] || !path.isAbsolute(options[name])) throw Error(`--${name} must be an absolute path outside the checkout`);
  const location = await realpath(name === 'output' ? path.dirname(options[name]) : options[name]);
  if (location === root || location.startsWith(root + path.sep)) throw Error(`--${name} must stay outside the checkout`);
}
const wasm = await readFile(options.wasm ?? 'web/core.wasm');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const server = spawn('python3', ['-m', 'http.server', '8077', '--bind', '127.0.0.1', '--directory', 'web'], {stdio:'ignore'});
let browser;
const rows = [];
try {
  for (let attempt = 0; ; attempt++) {
    if (server.exitCode !== null || attempt === 100) throw Error('Cannot start local server; port 8077 must be free');
    try { if ((await fetch('http://127.0.0.1:8077/')).ok) break; } catch {}
    await sleep(50);
  }
  browser = await chromium.launch({headless:true, channel:options.channel});
  for (let run = 0; run < repeat; run++) {
    const page = await browser.newPage();
    await page.route('**/core.wasm', route => route.fulfill({contentType:'application/wasm', body:wasm}));
    await page.goto(`http://127.0.0.1:8077/?diagnostics${playback ? '' : '&manual'}`);
    await page.waitForFunction(() => !document.querySelector('#profile').disabled);
    await page.locator('#drive').selectOption('2');
    await page.locator('#file').setInputFiles(options.media);
    await page.waitForFunction(() => !document.querySelector('#file').disabled);
    if (playback) await page.locator('#pause').click();
    const worker = page.workers().find(worker => worker.url().includes('emulator-worker.js'));
    async function inspect(fn, argument) {
      await worker.evaluate(() => pc98.beginInspection());
      try { return await worker.evaluate(fn, argument); }
      finally { await worker.evaluate(() => pc98.endInspection()); }
    }
    async function advance(ms) {
      await inspect(ms => {
        for (let at = 0; at < ms; at += 10) {
          pc98.advance(Math.min(10, ms-at)*1000); pc98.core.audio_clear();
        }
        if (pc98.stats().error) throw Error(pc98.stats().error);
      }, ms);
    }
    async function key(scan) {
      await worker.evaluate(scan => pc98.core.key(scan, 1), scan); await advance(50);
      await worker.evaluate(scan => pc98.core.key(scan, 0), scan); await advance(10);
    }
    // PC-98 scans; input still travels through the guest keyboard/IRQ path.
    const scans = {c:0x2b,d:0x1f,n:0x2e,a:0x1d,p:0x19,'2':2,' ':0x34};
    async function command(text) {
      for (const char of text) await key(scans[char]);
      await key(0x1c); await advance(1000);
    }
    await advance(12000);
    await command('cd nanpa2'); await command('nanpa2');
    await key(0x1c); await advance(1000); // Select the color display.
    if (options.gameplay === '1') {
      // Finish the opening, accept the default name through the guest mouse,
      // then measure the prologue's background, portrait and music loading.
      for (let n = 0; n < 4; n++) await advance(10000);
      async function mouse(dx, dy, buttons = 0, ms = 30) {
        await worker.evaluate(args => pc98.core.mouse(...args), [dx,dy,buttons]);
        await advance(ms);
      }
      for (let n = 0; n < 8; n++) await mouse(-100,-100);
      for (let n = 0; n < 5; n++) await mouse(100,50);
      await mouse(20,0,1,100); await mouse(0,0,0,1000);
    }
    let result;
    if (playback) {
      await page.locator('#sound').click(); await page.locator('#pause').click();
      await sleep(1500);
      const a = await page.evaluate(() => pc98.stats()), began = performance.now();
      const polls = [];
      for (let n = 0; n < (options.gameplay === '1' ? 48 : 16); n++) {
        if (options.gameplay === '1') {
          await worker.evaluate(() => pc98.core.mouse(0,0,1)); await sleep(80);
          await worker.evaluate(() => pc98.core.mouse(0,0,0)); await sleep(400);
        } else await sleep(500);
        polls.push(await page.evaluate(() => pc98.stats()));
      }
      const b = polls.at(-1), delta = key => b[key]-a[key];
      result = {wallMs:performance.now()-began, guestSeconds:delta('guestSeconds'),
        underflows:delta('underflows'), overflows:delta('overflows'), droppedSamples:delta('droppedSamples'),
        producedSamples:delta('producedSamples'), consumedSamples:delta('consumedSamples'),
        sequenceErrors:delta('sequenceErrors'), checksumErrors:delta('checksumErrors'),
        queueMin:Math.min(...polls.map(s => s.queuedSamples)), queueMax:Math.max(...polls.map(s => s.queuedSamples)),
        sliceMeanMs:(b.videoStats.guestSliceMs-a.videoStats.guestSliceMs)/(b.videoStats.guestSlices-a.videoStats.guestSlices),
        frames:b.presentationStats.frames-a.presentationStats.frames,
        reads:delta('hddReads'), keys:delta('fmKeyEdges'), irqs:delta('soundIrqEdges'), error:b.error};
      if (b.audioState !== 'running' || !b.running || b.audioEpoch !== a.audioEpoch) throw Error('Playback stopped or flushed');
    } else {
      const parts = []; let pcmHash = 2166136261;
      for (let part = 0; part < (options.gameplay === '1' ? 3 : 1); part++) {
        const block = await inspect(({gameplay, part, seed}) => {
          const a = pc98.stats(), began = performance.now(); let samples = 0, pcmHash = seed;
          for (let at = 0; at < 800; at++) {
            const n = part*800+at;
            if (gameplay && n % 48 === 0) pc98.core.mouse(0,0,1);
            if (gameplay && n % 48 === 8) pc98.core.mouse(0,0,0);
            pc98.advance(10000);
            const count = pc98.core.audio_len(), ptr = pc98.core.audio_ptr() >>> 0; samples += count;
            for (const bits of new Uint32Array(pc98.core.memory.buffer,ptr,count)) pcmHash = Math.imul(pcmHash ^ bits,16777619) >>> 0;
            pc98.core.audio_clear();
          }
          const wallMs = performance.now()-began, b = pc98.stats(); let frameHash = 2166136261;
          const ptr = pc98.core.frame() >>> 0;
          for (const byte of new Uint8Array(pc98.core.memory.buffer,ptr,640*400*4)) frameHash = Math.imul(frameHash ^ byte,16777619) >>> 0;
          return {wallMs, guestSeconds:b.guestSeconds-a.guestSeconds, instructions:b.completedInstructions-a.completedInstructions,
            cycles:b.cpuCycles-a.cpuCycles, reads:b.hddReads-a.hddReads, keys:b.fmKeyEdges-a.fmKeyEdges,
            irqs:b.soundIrqEdges-a.soundIrqEdges, samples, pcmHash, frameHash, error:b.error};
        }, {gameplay:options.gameplay === '1', part, seed:pcmHash});
        parts.push(block); pcmHash = block.pcmHash;
      }
      result = {...parts.at(-1)};
      for (const key of ['wallMs','guestSeconds','instructions','cycles','reads','keys','irqs','samples']) {
        result[key] = parts.reduce((sum, part) => sum+part[key], 0);
      }
    }
    if (result.error) throw Error(result.error);
    if (!result.keys || (options.gameplay === '1' && !result.reads)) {
      throw Error('Expected game music and scene loading did not run; check the clean installed HDD and setup');
    }
    rows.push(result); console.log(JSON.stringify({run, ...result})); await page.close();
  }
  await writeFile(options.output, JSON.stringify({context:{date:new Date().toISOString(), browser:browser.version(),
    channel:options.channel ?? 'chromium', node:process.version,
    os:`${os.type()} ${os.release()} ${os.arch()}`, cpu:os.cpus()[0].model,
    wasmSha256:sha(wasm), mediaSha256:sha(await readFile(options.media)),
    harnessSha256:sha(await readFile(new URL(import.meta.url))), playback, gameplay:options.gameplay === '1', repeat}, rows}, null, 2)+'\n', {flag:'wx'});
} finally { await browser?.close(); server.kill(); }
