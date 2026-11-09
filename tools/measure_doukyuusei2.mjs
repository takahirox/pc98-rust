// Opt-in local measurement; the installed game is never copied into web/.
// See docs/doukyuusei2-performance.md for installation and interpretation.
import {chromium, firefox} from '@playwright/test';
import {createServer} from 'node:http';
import {readFile, realpath, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
import {workerProfile} from '../tests/browser/helpers/worker-profile.js';

const options = Object.fromEntries(process.argv.slice(2).map(value => value.replace(/^--/, '').split('=')));
options.jit ??= '0';
const repeat = Number(options.repeat ?? 3), playback = options.playback === '1';
const engine = options.browser ?? 'chromium';
if(options.jit!==undefined&&!['0','1'].includes(options.jit))throw Error('Expected --jit=0|1');
if (!Number.isInteger(repeat) || repeat < 1 || repeat > 10) throw Error('Expected --repeat=1..10');
if (!['chromium','firefox'].includes(engine) || engine === 'firefox' && (options.channel || options.profile === '1')) {
  throw Error('Use --browser=chromium|firefox; channel selection and host profiles require Chromium');
}
if (options.channel && options.channel !== 'chrome') throw Error('Use --channel=chrome for installed Google Chrome, or omit it for pinned Chromium');
// Playwright 1.56.1's patched Firefox observes Wasm globals and disables
// optimizing compilation. Its starvation is not ordinary Firefox performance.
if (engine === 'firefox' && options.debugTier !== '1') {
  throw Error('Pinned Firefox automation forces debuggable baseline Wasm. Use the stock-browser reproduction control, or --debugTier=1 only for deliberate starvation diagnostics.');
}
const root = await realpath('.');
for (const name of ['media', 'output']) {
  if (!options[name] || !path.isAbsolute(options[name])) throw Error(`--${name} must be an absolute path outside the checkout`);
  const location = await realpath(name === 'output' ? path.dirname(options[name]) : options[name]);
  if (location === root || location.startsWith(root + path.sep)) throw Error(`--${name} must stay outside the checkout`);
}
const wasm = await readFile(options.wasm ?? 'web/core.wasm');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
// Freeze all executable assets before setup, including the lazily loaded
// AudioWorklet. Later source edits cannot mix revisions within a long run.
const runtime = await Promise.all(['app.js','emulator-worker.js','audio-worklet.js','render-worker.js',...(options.jit==='0'?[]:['jit.js'])]
  .map(async name => [name,await readFile(options.runtime ? path.join(options.runtime,name) : new URL('../web/'+name,import.meta.url))]));
const runtimeSha256 = Object.fromEntries(runtime.map(([name,body]) => [name,sha(body)]));
const harnessSha256 = sha(await readFile(new URL(import.meta.url)));
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const assets = new Map([...runtime,['core.wasm',wasm]]);
const webRoot = path.join(root,'web');
let routedAssets;
// Chromium's AudioWorklet module fetch can bypass Playwright page routing.
// Serve frozen executables over HTTP so every execution context gets them.
const server = createServer(async (request,response) => {
  try {
    const name = decodeURIComponent(new URL(request.url,'http://127.0.0.1').pathname).slice(1) || 'index.html';
    const location = path.resolve(webRoot,name);
    if (!location.startsWith(webRoot+path.sep)) { response.writeHead(404).end(); return; }
    const body = assets.has(name) ? assets.get(name) : await readFile(location);
    if (assets.has(name) && routedAssets) routedAssets[name]++;
    const mime = {'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html',
      '.json':'application/json','.svg':'image/svg+xml','.css':'text/css'}[path.extname(name)] ?? 'application/octet-stream';
    response.writeHead(200,{'Content-Type':mime,'Cache-Control':'no-store'}).end(body);
  } catch { response.writeHead(404).end(); }
});
let browser;
const rows = [];
try {
  await new Promise((resolve,reject) => { server.once('error',reject); server.listen(8077,'127.0.0.1',resolve); });
  browser = await ({chromium, firefox}[engine]).launch({headless:true, ...(options.channel ? {channel:options.channel} : {})});
  for (let run = 0; run < repeat; run++) {
    const page = await browser.newPage();
    routedAssets = Object.fromEntries([...assets.keys()].map(name => [name,0]));
    await page.goto(`http://127.0.0.1:8077/?diagnostics${playback ? '' : '&manual'}&jit=${options.jit==='0'?'0':'1'}`);
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
    // Diagnostic-only host samples; never compare these elapsed times with an
    // unprofiled run. Keep only aggregate host function counts, not guest data.
    const profile = options.profile === '1' ? workerProfile(browser,20000) : null;
    if (profile) await sleep(500);
    if (playback) {
      await page.locator('#sound').click(); await page.locator('#pause').click();
      await sleep(1500);
      const a = await page.evaluate(() => pc98.stats()), began = performance.now();
      if (options.visuals === '1') await page.evaluate(() => {
        const canvas = document.querySelector('#screen'), context = canvas.getContext('2d');
        const watch = window.pc98VisualWatch = {started:performance.now(), events:[], pending:null, overflow:false, samples:0, maxGapMs:0};
        let previous = '', reads = -1;
        let lastPoll = watch.started;
        watch.timer = setInterval(() => {
          if (watch.pending) return;
          const now = performance.now();
          watch.maxGapMs = Math.max(watch.maxGapMs,now-lastPoll); lastPoll = now; watch.samples++;
          const wallMs = performance.now()-watch.started, hashes = [];
          // Nine small patches cover the CG area above the dialogue panel.
          // Pixels stay in-page; only fingerprints and timing counters leave it.
          for (const y of [30,140,250]) for (const x of [70,310,550]) {
            let hash = 2166136261;
            for (const byte of context.getImageData(x,y,16,16).data) hash = Math.imul(hash ^ byte,16777619) >>> 0;
            hashes.push(hash);
          }
          watch.pending = pc98.stats().then(state => {
            const key = hashes.join(',');
            if (key !== previous || state.hddReads !== reads) {
              if (watch.events.length < 1024) watch.events.push({wallMs,stateWallMs:performance.now()-watch.started,
                guestSeconds:state.guestSeconds, reads:state.hddReads, cpuCycles:state.cpuCycles,
                guestSliceMs:state.videoStats.guestSliceMs, underflows:state.underflows,
                underflowRun:state.underflowRun, queuedSamples:state.queuedSamples,
                outstandingSamples:state.outstandingSamples, hashes});
              else watch.overflow = true;
            }
            previous = key; reads = state.hddReads;
          }).finally(() => { watch.pending = null; });
        },25);
      });
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
      if (options.visuals === '1') result.visuals = await page.evaluate(async () => {
        const watch = window.pc98VisualWatch; clearInterval(watch.timer); await watch.pending;
        return {sampleMs:25, samples:watch.samples, maxGapMs:watch.maxGapMs, overflow:watch.overflow, events:watch.events};
      });
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
            translatedInstructions:(b.jit?.instructions??0)-(a.jit?.instructions??0),
            cycles:b.cpuCycles-a.cpuCycles, reads:b.hddReads-a.hddReads, keys:b.fmKeyEdges-a.fmKeyEdges,
            irqs:b.soundIrqEdges-a.soundIrqEdges, samples, pcmHash, frameHash, error:b.error};
        }, {gameplay:options.gameplay === '1', part, seed:pcmHash});
        parts.push(block); pcmHash = block.pcmHash;
      }
      result = {...parts.at(-1)};
      for (const key of ['wallMs','guestSeconds','instructions','translatedInstructions','cycles','reads','keys','irqs','samples']) {
        result[key] = parts.reduce((sum, part) => sum+part[key], 0);
      }
    }
    if (result.error) throw Error(result.error);
    if (!result.keys || (options.gameplay === '1' && !result.reads)) {
      throw Error('Expected game music and scene loading did not run; check the clean installed HDD and setup');
    }
    const presented = await page.evaluate(() => pc98.stats());
    result.jit = presented.jit;
    result.videoMode = presented.videoMode; result.rendererFailure = presented.rendererFailure;
    result.routedAssets = routedAssets;
    for (const name of ['core.wasm','app.js','emulator-worker.js','render-worker.js',...(options.jit==='0'?[]:['jit.js']),...(playback ? ['audio-worklet.js'] : [])]) {
      if (!routedAssets[name]) throw Error(`Selected runtime asset was not intercepted: ${name}`);
    }
    if (profile) {
      const sampled = await profile, names = new Map(sampled.nodes.map(node => [node.id,node.callFrame.functionName]));
      const counts = new Map();
      for (const id of sampled.samples ?? []) counts.set(names.get(id),(counts.get(names.get(id)) ?? 0)+1);
      result.hostProfile = {samples:sampled.samples.length, functions:[...counts].sort((a,b) => b[1]-a[1]).slice(0,25)};
    }
    rows.push(result);
    const {visuals, hostProfile, ...summary} = result;
    console.log(JSON.stringify({run, ...summary, visualEvents:visuals?.events.length, profileSamples:hostProfile?.samples}));
    await page.close();
  }
  await writeFile(options.output, JSON.stringify({context:{date:new Date().toISOString(), browser:browser.version(),
    engine, channel:options.channel ?? engine, firefoxDebuggerBaseline:engine === 'firefox', node:process.version,
    os:`${os.type()} ${os.release()} ${os.arch()}`, cpu:os.cpus()[0].model,
    wasmSha256:sha(wasm), runtimeSha256, mediaSha256:sha(await readFile(options.media)),
    harnessSha256, playback, jit:options.jit!=='0', gameplay:options.gameplay === '1',
    profile:options.profile === '1', visuals:options.visuals === '1', repeat}, rows}, null, 2)+'\n', {flag:'wx'});
} finally { await browser?.close(); server.closeAllConnections(); server.close(); }
