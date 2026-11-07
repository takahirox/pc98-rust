// Opt-in, real release-Wasm comparison. Run after npm run build and fixture preparation.
// node tools/measure_render.mjs --browser=chromium --repeat=3 --seconds=8 --output=target/render-chromium.json
import {chromium,firefox} from '@playwright/test';
import {spawn} from 'node:child_process';
import {readFile,writeFile} from 'node:fs/promises';
import os from 'node:os';
import {createHash} from 'node:crypto';
const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
const engine=options.browser??'chromium', repeat=Number(options.repeat??3), seconds=Number(options.seconds??8);
const server=spawn('python3',['-m','http.server','8072','--bind','127.0.0.1','--directory','web'],{stdio:'ignore'});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
let browser;
async function waitFor(fn,timeout=30000) {
  const end=Date.now()+timeout;
  while(Date.now()<end) { if(await fn()) return; await sleep(20); }
  throw Error('Benchmark readiness/transition timeout');
}
const rows=[];
let presentationProbe;
try {
  await waitFor(async()=>{if(server.exitCode!==null)throw Error('Benchmark server exited; port 8072 must be free');try{return (await fetch('http://127.0.0.1:8072/')).ok;}catch{return false;}});
  browser=await ({chromium,firefox}[engine]).launch({headless:true});
  for(let run=0;run<repeat;run++) for(const workload of ['vm-startrail','ap-video','ap-music']) {
    // Alternate order to reduce warmup/thermal bias.
    for(const mode of run%2 ? ['worker','inline'] : ['inline','worker']) {
      const page=await browser.newPage();
      const began=Date.now();
      let bootStart;
      await page.goto(`http://127.0.0.1:8072/?diagnostics=1&renderer=${mode}`);
      await page.locator('#profile').waitFor({state:'visible'});
      await waitFor(()=>page.locator('#profile').isEnabled());
      if(workload==='vm-startrail') {
        await page.locator('#profile').selectOption('0');
        await waitFor(()=>page.locator('#game').isEnabled());
        await page.locator('#game').click();
        await waitFor(()=>page.evaluate(()=>pc98.text().then(s=>s.includes('STAR TRAIL'))));
      } else {
        const disk=workload==='ap-video'?'98videop':'music-scenes';
        await page.locator('#file').setInputFiles({name:`${disk}.hdm`,mimeType:'application/octet-stream',buffer:await readFile(`fixtures/dos-apps/${disk}.hdm`)});
        await waitFor(()=>page.locator('#boot').isEnabled());
        await page.locator('#boot').selectOption('0');
        await waitFor(()=>page.locator('#reset').isEnabled());
        bootStart=Date.now();
        await page.locator('#reset').click();
        await waitFor(()=>page.locator('#pause').isEnabled());
        if(workload==='ap-music') {
          await page.locator('#pause').click();
          await waitFor(()=>page.locator('#profile').isEnabled());
          const emu=page.workers().find(w=>w.url().includes('emulator-worker.js'));
          for(let i=0;i<2;i++) await emu.evaluate(()=>{
            pc98.beginInspection();
            try {for(let n=0;n<90;n++){pc98.advance(100000);pc98.core.audio_clear();}}
            finally {pc98.endInspection();}
          });
          await page.locator('#sound').click(); await page.locator('#pause').click();
          await page.locator('#screen').focus(); await page.keyboard.press('Enter');
          await waitFor(()=>page.evaluate(()=>pc98.stats().then(s=>s.fmKeyEdges>16)));
          await sleep(1500);
        } else {
          await page.locator('#sound').click();
        }
      }
      await page.evaluate(()=>{
        window.renderProbe=[];
        let previous=performance.now();
        window.renderProbeTimer=setInterval(()=>{
          const now=performance.now();renderProbe.push(Math.max(0,now-previous-16));previous=now;
        },16);
      });
      const start=await page.evaluate(()=>pc98.stats()), startWall=Date.now(), samples=[];
      if(start.videoMode!==mode) throw Error(`Requested ${mode}, got ${start.videoMode}: ${start.rendererFailure}`);
      let transitionMs=null, transitionGuestSeconds=null;
      const videoTransition=workload==='ap-video' ? (async()=>{
        await waitFor(()=>page.evaluate(()=>pc98.text().then(s=>s.includes('VIDEO RETURNED'))));
        transitionMs=Date.now()-bootStart;
        transitionGuestSeconds=(await page.evaluate(()=>pc98.stats())).guestSeconds-start.guestSeconds;
      })() : null;
      const picture=()=>page.locator('#screen').evaluate(canvas=>Array.from(canvas.getContext('2d').getImageData(8,100,32,1).data).join(','));
      for(let i=0;i<Math.ceil(seconds*4);i++) {
        if(i===Math.floor(seconds*2) && workload==='ap-music') {
          const old=await picture(), begin=Date.now(), guest=(await page.evaluate(()=>pc98.stats())).guestSeconds;
          await page.locator('#screen').focus(); await page.keyboard.press('Enter');
          await waitFor(async()=>await picture()!==old);
          transitionMs=Date.now()-begin;
          transitionGuestSeconds=(await page.evaluate(()=>pc98.stats())).guestSeconds-guest;
        }
        await sleep(250); samples.push(await page.evaluate(()=>pc98.stats()));
      }
      if(videoTransition) await videoTransition;
      const end=await page.evaluate(()=>pc98.stats()), wallMs=Date.now()-startWall;
      const delta=(group,key)=>(end[group]?.[key]??0)-(start[group]?.[key]??0);
      const frames=delta('presentationStats','frames'), snapshots=delta('videoStats','snapshots');
      const emu=page.workers().find(w=>w.url().includes('emulator-worker.js'));
      const render=page.workers().find(w=>w.url().includes('render-worker.js'));
      const timings=await page.evaluate(()=>{clearInterval(renderProbeTimer);return renderProbe.sort((a,b)=>a-b);});
      const row={run,workload,mode,wallMs,startupAndSetupMs:startWall-began,rendererStartupMs:end.rendererStartupMs,rendererFailure:end.rendererFailure,
        guestPerWallSecond:(end.guestSeconds-start.guestSeconds)/(wallMs/1000),
        transitionMs,transitionGuestSeconds,frames,snapshots,coalesced:delta('videoStats','coalesced'),stale:delta('presentationStats','stale'),
        guestSliceMeanMs:delta('videoStats','guestSliceMs')/delta('videoStats','guestSlices'),
        producerPreparationMeanMs:(delta('videoStats','snapshotMs')+delta('videoStats','inlineRasterMs')+delta('videoStats','copyMs'))/snapshots,
        snapshotMeanMs:delta('videoStats','snapshotMs')/snapshots,
        producerCopyMeanMs:delta('videoStats','copyMs')/snapshots,
        rasterMeanMs:(delta('videoStats','inlineRasterMs')+delta('renderStats','rasterMs'))/snapshots,
        renderCopyMeanMs:delta('renderStats','copyMs')/snapshots,
        messageMeanMs:delta('renderStats','transportMs')/snapshots,
        putMeanMs:delta('presentationStats','putMs')/frames,
        latencyMeanMs:delta('presentationStats','latencyMs')/frames,
        timerDelayP95Ms:timings[Math.floor(timings.length*.95)],timerDelayMaxMs:timings.at(-1),
        underflows:(end.underflows??0)-(start.underflows??0),overflows:(end.overflows??0)-(start.overflows??0),
        droppedSamples:end.droppedSamples-start.droppedSamples,
        queuedMin:Math.min(...samples.map(s=>s.queuedSamples??0)),queuedMax:Math.max(...samples.map(s=>s.queuedSamples??0)),
        emuWasmBytes:await emu.evaluate(()=>pc98.core.memory.buffer.byteLength),
        renderWasmBytes:render?(end.renderStats.wasmBytes??0):0,
        // Full snapshot bytes copied each frame, excluding immutable module code.
        snapshotBytes:mode==='worker'?(workload==='vm-startrail'?219952:547632):0,
        transferCapacityBytes:mode==='worker'?547632+640*400*4:640*400*4,
      };
      if(render && !presentationProbe) {
        const probe=() => {
          const canvas=typeof document==='undefined'?new OffscreenCanvas(640,400):document.createElement('canvas');
          canvas.width=640;canvas.height=400;
          const ctx=canvas.getContext('2d',{alpha:false});
          const pixels=new ImageData(640,400);
          for(let i=0;i<pixels.data.length;i++) pixels.data[i]=i%251;
          for(let i=0;i<10;i++)ctx.putImageData(pixels,0,0);
          let began=performance.now();
          for(let i=0;i<100;i++)ctx.putImageData(pixels,0,0);
          const putMs=(performance.now()-began)/100;
          let bitmapMs=null;
          if(canvas.transferToImageBitmap) {
            began=performance.now();
            for(let i=0;i<100;i++){ctx.putImageData(pixels,0,0);canvas.transferToImageBitmap().close();}
            bitmapMs=(performance.now()-began)/100;
          }
          return {putMs,bitmapMs};
        };
        presentationProbe={dom:await page.evaluate(probe),offscreen:await render.evaluate(probe)};
      }
      rows.push(row); console.log(JSON.stringify(row)); await page.close();
    }
  }
  const result={context:{date:new Date().toISOString(),wasmSha256:createHash('sha256').update(await readFile('web/core.wasm')).digest('hex'),engine,browser:browser.version(),os:`${os.type()} ${os.release()} ${os.arch()}`,cpu:os.cpus()[0].model,logicalCpus:os.cpus().length,repeat,seconds,presentationProbe},rows};
  await writeFile(options.output??`target/render-${engine}.json`,JSON.stringify(result,null,2)+'\n');
} finally {await browser?.close();server.kill();}
