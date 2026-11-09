// Opt-in local ADV98 benchmark. No downloads, frames, PCM, guest text or traces
// are saved. See docs/runtime-performance.md for rights, setup and interpretation.
import {chromium,firefox} from '@playwright/test';
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {readFile,writeFile,realpath} from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {createHash} from 'node:crypto';
import {workerProfile} from '../tests/browser/helpers/worker-profile.js';

const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
const engine=options.browser??'chromium', repeat=Number(options.repeat??3), seconds=Number(options.seconds??8);
if(!['chromium','firefox'].includes(engine) || !Number.isInteger(repeat) || repeat<1 || repeat>20 || !Number.isFinite(seconds) || seconds<4 || seconds>60)
  throw Error('Expected --browser=chromium|firefox, --repeat=1..20, --seconds=4..60');
// The pinned Firefox automation forces debuggable baseline Wasm, even without
// an explicit profiler. Keep that induced condition out of normal comparisons.
if(engine==='firefox' && options.debugTier!=='1')
  throw Error('Pinned Firefox automation forces debuggable baseline Wasm. Use a stock browser for normal performance, or --debugTier=1 only for deliberate starvation diagnostics.');
const root=await realpath('.');
const harnessSha256=createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex');
for(const name of ['media','output']) {
  if(!options[name] || !path.isAbsolute(options[name])) throw Error(`--${name} must be an absolute path outside the checkout`);
  const resolved=await realpath(name==='output'?path.dirname(options[name]):options[name]);
  if(resolved===root || resolved.startsWith(root+path.sep)) throw Error(`--${name} must stay outside the checkout`);
}
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const execute=promisify(execFile);
async function hostUsage(pid) {
  // Only the browser launched here and its descendants. No command lines,
  // environment, file paths or unrelated host processes enter the report.
  if(process.platform==='win32')return null;
  const pids=[pid];
  for(let i=0;i<pids.length;i++) {
    try {const {stdout}=await execute('pgrep',['-P',String(pids[i])]);pids.push(...stdout.trim().split(/\s+/).map(Number));}
    catch(error){if(error.code!==1)throw error;}
  }
  const {stdout}=await execute('ps',['-o','time=,rss=','-p',pids.join(',')]);
  let cpuSeconds=0,rssBytes=0;
  for(const line of stdout.trim().split('\n')) {
    const [time,rss]=line.trim().split(/\s+/),parts=time.split(':').map(Number);
    cpuSeconds+=parts.reduce((a,b)=>a*60+b,0);rssBytes+=Number(rss)*1024;
  }
  return {cpuSeconds,rssBytes};
}
async function waitFor(fn,timeout=30000) {
  const end=Date.now()+timeout;
  while(Date.now()<end) {if(await fn())return;await sleep(20);}
  throw Error('Benchmark readiness/transition timeout');
}
const stats=page=>page.evaluate(()=>pc98.stats());
const enter=async page=>{await page.locator('#screen').focus();await page.keyboard.press('Enter');};
const fingerprint=page=>page.locator('#screen').evaluate(canvas=>{
  // Reduce in-page; never export captured game pixels, even to a local report.
  const pixels=canvas.getContext('2d').getImageData(8,100,32,1).data;
  let hash=2166136261;for(const value of pixels)hash=Math.imul(hash^value,16777619);
  return hash>>>0;
});
function percentile(histogram,p) {
  const target=histogram.reduce((a,b)=>a+b,0)*p;
  let count=0;for(let i=0;i<histogram.length;i++){count+=histogram[i];if(count>=target)return (i+1)/10;}
  return null;
}
const server=spawn('python3',['-m','http.server','8073','--bind','127.0.0.1','--directory','web'],{stdio:'ignore'});
let browser, browserServer;
const rows=[], capacity=[], profiles=[], mediaHashes={};
try {
  await waitFor(async()=>{if(server.exitCode!==null)throw Error('Port 8073 must be free');try{return (await fetch('http://127.0.0.1:8073/')).ok;}catch{return false;}});
  browserServer=await ({chromium,firefox}[engine]).launchServer({headless:true});
  browser=await ({chromium,firefox}[engine]).connect(browserServer.wsEndpoint());
  async function setup(medium,mode,manual=false) {
    const page=await browser.newPage();
    await page.goto(`http://127.0.0.1:8073/?diagnostics&renderer=${mode}${manual?'&manual':''}`);
    await waitFor(()=>page.locator('#profile').isEnabled());
    const name=`music-scenes.${medium==='hdd'?'hdi':'hdm'}`;
    const bytes=await readFile(path.join(options.media,name));
    mediaHashes[name]=createHash('sha256').update(bytes).digest('hex');
    await page.locator('#file').setInputFiles({name,mimeType:'application/octet-stream',buffer:bytes});
    await waitFor(()=>page.locator('#boot').isEnabled());
    await page.locator('#boot').selectOption(medium==='hdd'?'1':'0');
    await waitFor(()=>page.locator('#reset').isEnabled());
    await page.locator('#reset').click();
    await waitFor(()=>page.locator('#profile').isEnabled());
    await page.locator('#pause').click();
    const worker=page.workers().find(w=>w.url().includes('emulator-worker.js'));
    for(let i=0;i<2;i++)await worker.evaluate(()=>{
      pc98.beginInspection();
      try {for(let n=0;n<90;n++){pc98.advance(100000);pc98.core.audio_clear();}}
      finally {pc98.endInspection();}
      if(pc98.stats().error)throw Error('Guest failed during setup');
    });
    if((await stats(page)).fmKeyEdges!==0)throw Error('Setup passed the silent title');
    return {page,worker};
  }
  for(let run=0;run<repeat;run++)for(const medium of ['floppy','hdd']) {
    for(const mode of options.capacityOnly==='1'?[]:run%2?['worker','inline']:['inline','worker']) {
      const {page,worker}=await setup(medium,mode);
      await page.locator('#sound').click();await page.locator('#pause').click();await enter(page);
      await waitFor(async()=>(await stats(page)).fmKeyEdges>16);
      await sleep(1500);
      const hostStart=await hostUsage(browserServer.process().pid);
      const start=await stats(page), started=performance.now(), samples=[];
      if(start.videoMode!==mode)throw Error('Renderer fallback invalidates A/B');
      let transitionMs,transitionGuestSeconds;
      for(let i=0;i<Math.ceil(seconds*4);i++) {
        if(i===Math.floor(seconds*2)) {
          const before=await fingerprint(page), guest=(await stats(page)).guestSeconds, began=performance.now();
          await enter(page);
          try {
            await waitFor(async()=>await fingerprint(page)!==before,1000);
            transitionMs=performance.now()-began;transitionGuestSeconds=(await stats(page)).guestSeconds-guest;
          } catch {
            // A starved producer can defer video even after the guest read the
            // scene. Retain that negative result instead of waiting indefinitely
            // or excluding the slow browser from the report.
            transitionMs=null;transitionGuestSeconds=null;
          }
        }
        await sleep(250);samples.push(await stats(page));
      }
      const end=await stats(page),wallMs=performance.now()-started;
      const hostEnd=await hostUsage(browserServer.process().pid);
      if(end.error || !end.running || end.audioEpoch!==start.audioEpoch)throw Error('Guest/audio stopped during measurement');
      const d=key=>end[key]-start[key], v=(group,key)=>end[group][key]-start[group][key];
      const histogram=end.sliceHistogram.map((n,i)=>n-start.sliceHistogram[i]);
      const guestSeconds=d('guestSeconds'),frames=v('presentationStats','frames'),snapshots=v('videoStats','snapshots');
      rows.push({run,medium,mode,wallMs,guestSeconds,guestPerWallSecond:guestSeconds*1000/wallMs,
        browserCpuSeconds:hostEnd&&hostEnd.cpuSeconds-hostStart.cpuSeconds,browserRssBytes:hostEnd?.rssBytes,
        completedInstructions:d('completedInstructions'),instructionsPerSecond:d('completedInstructions')*1000/wallMs,
        cpuCycles:d('cpuCycles'),schedulerCycles:d('cycles'),reads:d('reads'),hddReads:d('hddReads'),transitionMs,transitionGuestSeconds,
        guestSliceMeanMs:v('videoStats','guestSliceMs')/v('videoStats','guestSlices'),guestSliceP95Ms:percentile(histogram,.95),guestSliceP99Ms:percentile(histogram,.99),
        guestExecutionDuty:v('videoStats','guestSliceMs')/wallMs,
        fps:frames*1000/wallMs,coalesced:v('videoStats','coalesced'),latencyMeanMs:v('presentationStats','latencyMs')/frames,
        preparationMeanMs:(v('videoStats','snapshotMs')+v('videoStats','copyMs')+v('videoStats','inlineRasterMs'))/snapshots,
        rasterMeanMs:(v('videoStats','inlineRasterMs')+(mode==='worker'?v('renderStats','rasterMs'):0))/snapshots,
        producedSamples:d('producedSamples'),consumedSamples:d('consumedSamples'),deviceSampleRate:end.deviceSampleRate,
        underflows:d('underflows'),overflows:d('overflows'),droppedSamples:d('droppedSamples'),
        queueMin:Math.min(...samples.map(s=>s.queuedSamples)),queueMax:Math.max(...samples.map(s=>s.queuedSamples)),
        fmKeyEdges:d('fmKeyEdges'),soundIrqEdges:d('soundIrqEdges'),irqAcks:d('irqAcks'),
        fmPitches:[...new Set(samples.map(s=>s.fmPitch))].sort((a,b)=>a-b),ssgPeriods:[...new Set(samples.map(s=>s.ssgPeriod))].sort((a,b)=>a-b),
        emuWasmBytes:await worker.evaluate(()=>pc98.core.memory.buffer.byteLength),renderWasmBytes:end.renderStats?.wasmBytes??0});
      if(options.profile==='1' && engine==='chromium' && run===0 && medium==='floppy' && mode==='worker') {
        const profile=await workerProfile(browser,4000),names=new Map(profile.nodes.map(n=>[n.id,n.callFrame.functionName]));
        const counts=new Map();for(const id of profile.samples??[])counts.set(names.get(id),(counts.get(names.get(id))??0)+1);
        profiles.push({medium,mode,totalSamples:profile.samples.length,functions:[...counts].sort((a,b)=>b[1]-a[1]).slice(0,25)});
      }
      await page.close();
    }
    // Separate capacity control: same fixed guest-time workload without pacing,
    // video or sink. Does not count as production playback evidence.
    const {page,worker}=await setup(medium,'worker',true);
    const result=await worker.evaluate(()=>{
      pc98.beginInspection();
      try {
        pc98.core.key(0x1c,1);pc98.advance(100000);pc98.core.key(0x1c,0);
        for(let i=0;i<19;i++){pc98.advance(100000);pc98.core.audio_clear();}
        const start=pc98.stats(),began=performance.now();let samples=0,pcmHash=2166136261;
        for(let i=0;i<400;i++) {
          if(i===200)pc98.core.key(0x1c,1);
          if(i===210)pc98.core.key(0x1c,0);
          // Drain every 10 ms, below the core's finite PCM queue capacity.
          pc98.advance(10000);
          const count=pc98.core.audio_len(),ptr=pc98.core.audio_ptr()>>>0;
          samples+=count;
          for(const bits of new Uint32Array(pc98.core.memory.buffer,ptr,count))pcmHash=Math.imul(pcmHash^bits,16777619)>>>0;
          pc98.core.audio_clear();
        }
        const wallMs=performance.now()-began,end=pc98.stats();
        if(end.error)throw Error('Capacity guest failed');
        const ptr=pc98.core.frame()>>>0;let frameHash=2166136261;
        for(const byte of new Uint8Array(pc98.core.memory.buffer,ptr,640*400*4))frameHash=Math.imul(frameHash^byte,16777619)>>>0;
        return {wallMs,guestSeconds:end.guestSeconds-start.guestSeconds,cpuCycles:end.cpuCycles-start.cpuCycles,
          completedInstructions:end.completedInstructions-start.completedInstructions,reads:end.reads-start.reads,hddReads:end.hddReads-start.hddReads,
          fmKeyEdges:end.fmKeyEdges-start.fmKeyEdges,soundIrqEdges:end.soundIrqEdges-start.soundIrqEdges,samples,pcmHash,frameHash};
      }finally{pc98.endInspection();}
    });
    capacity.push({run,medium,...result,guestPerWallSecond:result.guestSeconds*1000/result.wallMs});await page.close();
    console.log(`Completed ${engine} run ${run+1}/${repeat}: ${medium}`);
  }
  const result={context:{revision:options.revision??'unspecified',date:new Date().toISOString(),
    wasmSha256:createHash('sha256').update(await readFile('web/core.wasm')).digest('hex'),
    workerSha256:createHash('sha256').update(await readFile('web/emulator-worker.js')).digest('hex'),
    harnessSha256,mediaHashes,engine,firefoxDebuggerBaseline:engine==='firefox',browser:browser.version(),
    os:`${os.type()} ${os.release()} ${os.arch()}`,cpu:os.cpus()[0].model,logicalCpus:os.cpus().length,hostMemoryBytes:os.totalmem(),repeat,seconds,capacityOnly:options.capacityOnly==='1'},rows,capacity,profiles};
  await writeFile(options.output,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
}finally{await browser?.close();await browserServer?.close();server.kill();}
