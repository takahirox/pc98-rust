// Opt-in fixed-work comparison using an original MIT register loop, not game media.
import {chromium} from '@playwright/test';
import {createServer} from 'node:http';
import {readFile,realpath,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import os from 'node:os';
const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
const repeat=Number(options.repeat??3),units=5000000,root=await realpath('.');
if(!Number.isInteger(repeat)||repeat<1||repeat>10)throw Error('Expected --repeat=1..10');
if(!options.output||!path.isAbsolute(options.output))throw Error('--output must be absolute');
const folder=await realpath(path.dirname(options.output));
if(folder===root||folder.startsWith(root+path.sep))throw Error('Keep the working report outside checkout');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const assets=new Map(await Promise.all(['core.wasm','app.js','emulator-worker.js','render-worker.js','audio-worklet.js','jit.js']
  .map(async name=>[name,await readFile(path.join(root,'web',name))])));
const server=createServer(async(req,res)=>{try{
  const name=decodeURIComponent(new URL(req.url,'http://localhost').pathname).slice(1)||'index.html';
  const file=path.resolve(root,'web',name);if(!file.startsWith(path.join(root,'web')+path.sep))throw Error();
  const body=assets.get(name)??await readFile(file);
  res.writeHead(200,{'Content-Type':{'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html','.css':'text/css','.json':'application/json'}[path.extname(name)]??'application/octet-stream','Cache-Control':'no-store'}).end(body);
}catch{res.writeHead(404).end();}});
let browser;const rows=[];
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(18152,'127.0.0.1',resolve);});
  browser=await chromium.launch({channel:'chrome',headless:true});
  for(let run=0;run<repeat;run++)for(const enabled of run%2?[true,false]:[false,true]){
    const page=await browser.newPage();
    try{
      await page.goto('http://127.0.0.1:18152/?diagnostics&manual&jit='+(enabled?1:0));
      await page.waitForFunction(()=>!document.querySelector('#profile').disabled);
      const disk=Buffer.alloc(1261568);disk.set([0xfa,0x01,0xd8,0x31,0xc1,0x46,0x4a,0x09,0xc3,0x83,0xc7,1,0xeb,0xf3]);
      await page.locator('#drive').selectOption('0');
      await page.locator('#file').setInputFiles({name:'jit.hdm',mimeType:'application/octet-stream',buffer:disk});
      await page.waitForFunction(()=>!document.querySelector('#file').disabled);
      const worker=page.workers().find(w=>w.url().includes('emulator-worker.js'));
      await worker.evaluate(()=>pc98.core.set_boot_source(0));await page.locator('#reset').click();
      async function inspect(fn,arg){await worker.evaluate(()=>pc98.beginInspection());try{return await worker.evaluate(fn,arg);}finally{await worker.evaluate(()=>pc98.endInspection());}}
      let warm=0;while(warm<200000)warm+=await inspect(cap=>{const done=pc98.core.step_scheduler(80000,cap);pc98.core.audio_clear();return done;},Math.min(10000,200000-warm));
      const warmed=await page.evaluate(async()=>(await pc98.stats()).jit);
      if(enabled&&!(warmed.compiled>0&&warmed.failures===0))throw Error('JIT did not warm successfully');
      const result=await inspect(units=>{
        const core=pc98.core,started=performance.now();let done=0,samples=0,pcmHash=2166136261;
        while(done<units){
          done+=core.step_scheduler(80000,Math.min(100000,units-done));
          const count=core.audio_len(),p=core.audio_ptr()>>>0;samples+=count;
          for(const bits of new Uint32Array(core.memory.buffer,p,count))pcmHash=Math.imul(pcmHash^bits,16777619)>>>0;
          core.audio_clear();if(core.metric(4))throw Error(pc98.stats().error);
        }
        const wallMs=performance.now()-started,p=core.jit_layout()>>>0;
        const [r,f,ip]=new Uint32Array(core.memory.buffer,p,3),view=new DataView(core.memory.buffer);
        return {wallMs,done,state:{registers:Array.from(new Uint16Array(core.memory.buffer,r,8)),flags:view.getUint16(f,true),ip:view.getUint32(ip,true),
          metrics:[0,1,2,5,19,20,26,27,28,30].map(i=>core.metric(i)),samples,pcmHash},jit:pc98.stats().jit};
      },units);
      rows.push({run,enabled,warmed,...result});console.log(JSON.stringify({run,enabled,wallMs:result.wallMs,jit:result.jit}));
    }finally{await page.close();}
  }
  const reference=JSON.stringify(rows[0].state);
  if(rows.some(r=>JSON.stringify(r.state)!==reference))throw Error('Fixed-work states differ');
  await writeFile(options.output,JSON.stringify({context:{date:new Date().toISOString(),browser:browser.version(),node:process.version,
    os:os.type()+' '+os.release()+' '+os.arch(),cpu:os.cpus()[0].model,units,warmUnits:200000,repeat,
    sourceHashes:Object.fromEntries([...assets].map(([n,b])=>[n,sha(b)])),harnessSha256:sha(await readFile(new URL(import.meta.url)))},rows},null,2)+'\n',{flag:'wx'});
}finally{await browser?.close();server.closeAllConnections();server.close();}
