// Private-media A/B probe. Output contains guest state; keep it outside Git.
import {chromium} from '@playwright/test';
import {createServer} from 'node:http';
import {realpath,readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {resolve,dirname,isAbsolute,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const opts=Object.fromEntries(process.argv.slice(2).map(x=>x.replace(/^--/,'').split('=')));
if(!opts.baseline||!opts.candidate||!opts.media||!opts.output)throw Error('Required: --baseline=FILE --candidate=FILE --media=PRIVATE_HDI --output=NEW_JSON [--repeat=3]');
const repeat=Number(opts.repeat??3);
if(!Number.isInteger(repeat)||repeat<1||repeat>20)throw Error('Use --repeat=1..20');
const checkout=await realpath(root);
const harnessSha256=createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex');
const base=opts.baseline,candidate=opts.candidate;
for(const key of ['media', 'output']){
 if(!isAbsolute(opts[key]))throw Error('--'+key+' must be absolute');
 const location=await realpath(key==='output'?dirname(opts[key]):opts[key]);
 if(location===checkout||location.startsWith(checkout+sep))throw Error('--'+key+' must stay outside the checkout');
}
const sha=b=>createHash('sha256').update(b).digest('hex');
const paths={'base.wasm':base,'candidate.wasm':candidate,'jit.js':opts.jitSource??root+'/web/jit.js','game.hdi':opts.media};
const assets=new Map(await Promise.all(Object.entries(paths).map(async([k,p])=>[k,await readFile(p)])));
for(const name of ['block-jit.mjs','block-compiler.mjs'])assets.set('tools/lib/'+name,await readFile(root+'/tools/lib/'+name));assets.set('web/jit.js',assets.get('jit.js'));
const worker=`
import {WasmJit} from '/jit.js';import {BlockJit} from '/tools/lib/block-jit.mjs';
const sleep=()=>new Promise(r=>setTimeout(r,0));
const modules=await Promise.all(['base.wasm','candidate.wasm'].map(async p=>WebAssembly.compile(await(await fetch('/'+p)).arrayBuffer())));
const disk=new Uint8Array(await(await fetch('/game.hdi')).arrayBuffer());
const scans={};for(const [s,start] of [['1234567890',1],['qwertyuiop',16],['asdfghjkl',29],['zxcvbnm',41]])for(let i=0;i<s.length;i++)scans[s[i]]=start+i;scans[' ']=52;
function message(c){let p=c.message(0);return new TextDecoder().decode(new Uint8Array(c.memory.buffer,p,c.message_len()));}
function put(c,b){let p=c.input_buffer(b.length)>>>0;new Uint8Array(c.memory.buffer,p,b.length).set(b);}
function make(module,index){const c=new WebAssembly.Instance(module,{}).exports;c.set_profile(1);c.set_boot_source(1);put(c,disk.subarray(0,32));if(!c.hdi_begin(disk.length))throw Error('begin');for(let at=0;at<disk.length;at+=1048576){put(c,disk.subarray(at,at+1048576));if(!c.hdi_append())throw Error('append');}if(!c.hdi_commit())throw Error('commit');c.reset();return {c,j:index===0?new WasmJit(c):new BlockJit(c),samples:0,pcmHash:2166136261,time:0,setupActiveMs:0};}
const hash=bytes=>{let h=2166136261;for(const v of bytes)h=Math.imul(h^v,16777619)>>>0;return h;};
function state(m){const c=m.c,[r,f,ip]=new Uint32Array(c.memory.buffer,c.jit_layout()>>>0,3);return {registers:Array.from(new Uint16Array(c.memory.buffer,r,8)),flags:new DataView(c.memory.buffer).getUint16(f,true),ip:new DataView(c.memory.buffer).getUint32(ip,true),cs:c.metric(17),metrics:[0,1,2,5,6,7,9,10,14,15,19,20,23,26,27,28,30].map(i=>c.metric(i)),samples:m.samples,pcmHash:m.pcmHash,ramHash:(()=>{let h=2166136261;for(let at=0;at<0xa0000;at++)h=Math.imul(h^c.peek(at),16777619)>>>0;return h;})(),frameHash:hash(new Uint8Array(c.memory.buffer,c.frame()>>>0,1024000))};}
function slice(m,measure){const c=m.c,begin=performance.now();m.j.step(80000,100000);const count=c.audio_len();if(measure){m.samples+=count;for(const v of new Uint32Array(c.memory.buffer,c.audio_ptr()>>>0,count))m.pcmHash=Math.imul(m.pcmHash^v,16777619)>>>0;}c.audio_clear();if(measure)m.time+=performance.now()-begin;else m.setupActiveMs+=performance.now()-begin;if(c.metric(4))throw Error(message(c));}
async function settle(ms){while(ms.some(m=>m.j.pending))await sleep();if(ms.some(m=>m.j.failed))throw Error('compile failed '+ms.map(m=>m.j.compileError??'').join('|'));}
const results=[];
for(let repeat=0;repeat<${repeat};repeat++){
 const ms=modules.map(make);let turn=repeat;
 async function advance(amount){for(let i=0;i<amount;i+=10){for(const k of turn++%2?[1,0]:[0,1])slice(ms[k],false);await settle(ms);}}
 async function key(scan){for(const m of ms)m.c.key(scan,1);await advance(50);for(const m of ms)m.c.key(scan,0);await advance(10);}
 async function command(s){for(const c of s)await key(scans[c]);await key(28);await advance(1000);}
 async function mouse(dx,dy,buttons=0,amount=30){for(const m of ms)m.c.mouse(dx,dy,buttons);await advance(amount);}
 await advance(12000);await command('cd nanpa2');await command('nanpa2');await key(28);await advance(1000);for(let i=0;i<4;i++)await advance(10000);
 for(let i=0;i<8;i++)await mouse(-100,-100);for(let i=0;i<5;i++)await mouse(100,50);await mouse(20,0,1,100);await mouse(0,0,0,1000);
 const before=ms.map(state);if(JSON.stringify(before[0])!==JSON.stringify(before[1]))throw Error('setup differs');
 const jitBefore=ms.map(m=>m.j.stats());
 const partials=[];
 for(let n=0;n<2400;n++){
  for(const m of ms){if(n%48===0)m.c.mouse(0,0,1);if(n%48===8)m.c.mouse(0,0,0);}
  for(const k of turn++%2?[1,0]:[0,1])slice(ms[k],true);
  await settle(ms);
  if(n%800===799){const s=ms.map(state);if(JSON.stringify(s[0])!==JSON.stringify(s[1]))throw Error('measured state differs at '+n);partials.push(s[0]);}
 }
 const rows=ms.map((m,k)=>({time:m.time,setupActiveMs:m.setupActiveMs,state:state(m),jit:m.j.stats(),jitBefore:jitBefore[k],before:before[k]}));
 const result={repeat,rows,partials};results.push(result);postMessage({progress:repeat,times:rows.map(r=>r.time)});
}
postMessage({results});
`;
assets.set('bench.js',Buffer.from(worker));
const server=createServer((req,res)=>{const name=new URL(req.url,'http://localhost').pathname.slice(1);if(!name){res.writeHead(200,{'Content-Type':'text/html'}).end('<!doctype html><title>Owned comparison</title>');return;}const b=assets.get(name);if(!b){res.writeHead(404).end();return;}res.writeHead(200,{'Content-Type':name.endsWith('.wasm')?'application/wasm':(name.endsWith('.js')||name.endsWith('.mjs'))?'text/javascript':'application/octet-stream','Cache-Control':'no-store'}).end(b);});
let browser,owner;let failed=false;
try{await new Promise((r,j)=>{server.once('error',j);server.listen(0,'127.0.0.1',r);});owner=await chromium.launchServer({channel:'chrome',headless:true});browser=await chromium.connect(owner.wsEndpoint());const page=await browser.newPage();page.on('console',m=>console.log(m.text()));await page.goto('http://127.0.0.1:'+server.address().port+'/');const result=await page.evaluate(()=>new Promise((resolve,reject)=>{const worker=new Worker('/bench.js',{type:'module'});worker.onerror=e=>reject(Error(e.message));worker.onmessage=e=>{if(e.data.results){worker.terminate();resolve(e.data);}else console.log(JSON.stringify(e.data));};}));await writeFile(opts.output,JSON.stringify({context:{scene:'progressing-scene',date:new Date().toISOString(),browser:browser.version(),os:os.release(),arch:os.arch(),cpu:os.cpus()[0].model,hashes:Object.fromEntries([...assets].filter(([k])=>k!=='game.hdi').map(([k,v])=>[k,sha(v)])),mediaSha256:sha(assets.get('game.hdi')),harnessSha256},...result},null,2),{flag:'wx'});}catch(error){failed=true;console.error(error.stack??String(error));}finally{server.closeAllConnections();server.close();if(owner){const timer=setTimeout(()=>owner.kill().catch(()=>{}),10000);try{await owner.close();}finally{clearTimeout(timer);}}process.exit(failed?1:0);}
