// Stationary private-scene reference control, including software raster cost.
import {chromium} from '@playwright/test';
import {readFile,writeFile,realpath,mkdir} from 'node:fs/promises';import {createServer} from 'node:http';import {createHash} from 'node:crypto';
import {isAbsolute,dirname,sep} from 'node:path';import {fileURLToPath} from 'node:url';
const opt=Object.fromEntries(process.argv.slice(2).map(s=>s.replace(/^--/,'').split('=')));
const root=new URL('../',import.meta.url);
for(const key of ['wasm','media','output'])if(!opt[key])throw Error('Required: --wasm=FILE --media=PRIVATE_HDI --output=NEW_DIR [--block=1]');
const checkout=await realpath(fileURLToPath(root));
for(const key of ['media','output']){if(!isAbsolute(opt[key]))throw Error('--'+key+' must be absolute');const p=await realpath(key==='output'?dirname(opt[key]):opt[key]);if(p===checkout||p.startsWith(checkout+sep))throw Error('--'+key+' must stay outside the checkout');}
await mkdir(opt.output);
const paths={'core.wasm':opt.wasm,'jit.js':fileURLToPath(new URL('web/jit.js',root)),'game.hdi':opt.media};
const assets=new Map(await Promise.all(Object.entries(paths).map(async([k,p])=>[k,await readFile(p)])));
for(const name of ['block-jit.mjs','block-compiler.mjs'])assets.set('tools/lib/'+name,await readFile(new URL('tools/lib/'+name,root)));assets.set('web/jit.js',assets.get('jit.js'));
const worker=`import {WasmJit} from '/jit.js';import {BlockJit} from '/tools/lib/block-jit.mjs';
const coldStart=performance.now();
const mod=await WebAssembly.compile(await(await fetch('/core.wasm')).arrayBuffer()),disk=new Uint8Array(await(await fetch('/game.hdi')).arrayBuffer());
const c=new WebAssembly.Instance(mod,{}).exports,r=new WebAssembly.Instance(mod,{}).exports;
const put=(core,b)=>{const p=core.input_buffer(b.length)>>>0;new Uint8Array(core.memory.buffer,p,b.length).set(b);};
c.set_profile(1);c.set_boot_source(1);put(c,disk.subarray(0,32));if(!c.hdi_begin(disk.length))throw Error('begin');for(let at=0;at<disk.length;at+=1048576){put(c,disk.subarray(at,at+1048576));if(!c.hdi_append())throw Error('append');}if(!c.hdi_commit())throw Error('commit');c.reset();const j=${opt.block==='1'?'new BlockJit(c)':'new WasmJit(c)'};
let time=0,cpuTime=0,rasterTime=0,samples=0,frames=0,renderAt=0;const output=new Uint8Array(1024000);
function draw(measure){const t=performance.now(),p=c.video_snapshot()>>>0,n=c.video_snapshot_len();put(r,new Uint8Array(c.memory.buffer,p,n));const out=r.video_render(n)>>>0;if(!out)throw Error('snapshot');output.set(new Uint8Array(r.memory.buffer,out,1024000));if(measure){const ms=performance.now()-t;time+=ms;rasterTime+=ms;frames++;}}
async function slice(clocks,measure){const start=performance.now();${opt.jit==='0'?'c.step_scheduler(clocks,100000)':'j.step(clocks,100000)'};const n=c.audio_len();c.audio_clear();if(measure){samples+=n;const ms=performance.now()-start;time+=ms;cpuTime+=ms;}while(j.pending)await new Promise(r=>setTimeout(r,0));if(j.failed||c.metric(4))throw Error('guest/JIT error');}
async function advance(ms,measure=false){const target=c.metric(5)+ms*8000;while(c.metric(5)<target){await slice(Math.ceil(Math.min(80000,target-c.metric(5))),measure);if(c.metric(5)>=renderAt){draw(measure);renderAt=c.metric(5)+8000000/56.4;}}}
const scans={};for(const[s,start]of[['1234567890',1],['qwertyuiop',16],['asdfghjkl',29],['zxcvbnm',41]])for(let i=0;i<s.length;i++)scans[s[i]]=start+i;scans[' ']=52;
async function key(scan){c.key(scan,1);await advance(55);c.key(scan,0);await advance(20);}
async function command(s){for(const ch of s)await key(scans[ch]);await key(28);await advance(1000);}
async function mouse(x,y,buttons,ms){c.mouse(x,y,buttons);await advance(ms);}
const captures=[];function capture(name){draw(false);captures.push({name,bytes:output.slice()});}
function anchor(){let h=2166136261;for(let y=326;y<348;y++)for(let x=118;x<338;x++){const i=(y*640+x)*4;h=Math.imul(h^(Math.min(output[i],output[i+1],output[i+2])>200?1:0),16777619)>>>0;}return h;}

await advance(12000);capture('boot');await command('cd nanpa2');await command('nanpa2');await key(28);await advance(1000);capture('start');for(let i=0;i<4;i++)await advance(10000);capture('opening');for(let i=0;i<8;i++)await mouse(-100,-100,0,30);for(let i=0;i<5;i++)await mouse(100,50,0,30);await mouse(20,0,1,100);await mouse(0,0,0,1000);await advance(10000);capture('prologue');if(anchor()!==854969663)throw Error('scene anchor before');
const setupWallMs=performance.now()-coldStart;
const before=Object.fromEntries([0,5,7,9,23,26,27,28].map(k=>[k,c.metric(k)])),begin=c.metric(5),goal=begin+24*8000000,beginJit=j.stats();renderAt=begin;
const loopBegin=performance.now();while(c.metric(5)<goal){const elapsed=(c.metric(5)-begin)/8000;await slice(Math.ceil(Math.min(80000,goal-c.metric(5))),true);if(c.metric(5)>=renderAt){draw(true);renderAt+=8000000/56.4;}}
const loopMs=performance.now()-loopBegin;const after=Object.fromEntries(Object.keys(before).map(k=>[k,c.metric(Number(k))-before[k]]));capture('final');if(anchor()!==854969663)throw Error('scene anchor after');postMessage({captures:${opt.captures==='1'?'captures':'[]'},result:{setupWallMs,loopMs,guestSeconds:(c.metric(5)-begin)/8000000,wallMs:time,cpuMs:cpuTime,rasterMs:rasterTime,sourceSampleRate:48000,audioFrames:samples,videoFrames:frames,metrics:after,jit:j.stats(),beginJit}});`;
assets.set('worker.js',Buffer.from(worker));const server=createServer((req,res)=>{let name=new URL(req.url,'http://localhost').pathname.slice(1);if(!name){res.writeHead(200,{'Content-Type':'text/html'}).end('<!doctype html><title>Owned emulator comparison</title>');return;}if(name==='favicon.ico'){res.writeHead(204).end();return;}const b=assets.get(name);if(!b){res.writeHead(404).end();return;}res.writeHead(200,{'Content-Type':name.endsWith('.wasm')?'application/wasm':name.endsWith('.js')||name.endsWith('.mjs')?'text/javascript':'application/octet-stream','Cache-Control':'no-store'}).end(b);});
let browser,owner;let failed=false;try{await new Promise((r,j)=>{server.once('error',j);server.listen(0,'127.0.0.1',r);});owner=await chromium.launchServer({channel:'chrome',headless:true});browser=await chromium.connect(owner.wsEndpoint());const page=await browser.newPage();const pictures=[];await page.exposeFunction('captureFrame',async(name,bytes)=>{await writeFile(opt.output+'/'+name+'.rgba',Buffer.from(bytes));pictures.push(name);});await page.goto('http://127.0.0.1:'+server.address().port+'/');const result=await page.evaluate(()=>new Promise((resolve,reject)=>{const w=new Worker('/worker.js',{type:'module'});w.onerror=e=>reject(Error(e.message));w.onmessage=async e=>{if(e.data.result){for(const p of e.data.captures)await window.captureFrame(p.name,Array.from(p.bytes));w.terminate();resolve(e.data.result);}};}));const sha=b=>createHash('sha256').update(b).digest('hex');await writeFile(opt.output+'/result.json',JSON.stringify({browser:browser.version(),wasm:sha(assets.get('core.wasm')),jitSource:sha(assets.get('jit.js')),mode:opt.block==='1'?'block-experiment':'jit',result},null,2),{flag:'wx'});console.log(JSON.stringify(result));}catch(error){failed=true;console.error(error.stack??String(error));}finally{server.closeAllConnections();server.close();if(owner){const timer=setTimeout(()=>owner.kill().catch(()=>{}),10000);try{await owner.close();}finally{clearTimeout(timer);}}process.exit(failed?1:0);}
