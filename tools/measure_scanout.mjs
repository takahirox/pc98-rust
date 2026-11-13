// Opt-in comparison of frozen release-Wasm artifacts with original snapshots.
import {chromium} from '@playwright/test';
import {createServer} from 'node:http';
import {readFile,writeFile,realpath} from 'node:fs/promises';
import path from 'node:path';
import {createHash} from 'node:crypto';
import os from 'node:os';
const opts=Object.fromEntries(process.argv.slice(2).map(x=>x.replace(/^--/,'').split('=')));
const root=await realpath('.');
for(const name of ['baseline','candidate','output'])if(!opts[name]||!path.isAbsolute(opts[name]))throw Error('--'+name+' must be absolute');
const outputDirectory=await realpath(path.dirname(opts.output));
if(outputDirectory===root||outputDirectory.startsWith(root+path.sep))throw Error('Keep measurement output outside the checkout');
const paths={'base.wasm':opts.baseline,'candidate.wasm':opts.candidate};
const assets=new Map(await Promise.all(Object.entries(paths).map(async([k,p])=>[k,await readFile(p)])));
const worker=`
const cores=await Promise.all(['base.wasm','candidate.wasm'].map(async p=>(await WebAssembly.instantiate(await(await fetch('/'+p)).arrayBuffer(),{})).instance.exports));
const GRAPHICS=23344,TEXT=6960,GLYPHS=944;
function snapshot(profile,mode){
 const b=new Uint8Array(GRAPHICS+(profile?0x80000:0x30000)),v=new DataView(b.buffer);b.set([86,57,56,1,profile]);b[5]=(mode&31)|(profile&&mode%7===0?32:0)|64;b[6]=mode&1;
 for(let i=0;i<2;i++){const at=16+i*32;b.set([mode%9===0?0:1,80,16,1,1,14,1,1],at);v.setUint32(at+8,i?0x3fff-mode:4090,true);v.setUint32(at+12,4090,true);b.set([0xf0,0x0f,0x30,0x84,0,0,0x80,0x14],at+16);}
 for(let i=128;i<GLYPHS;i++)b[i]=(i*17+mode)&255;
 for(let i=GLYPHS;i<TEXT;i++)b[i]=(i^i>>>3)&255;
 for(let i=0;i<4096;i++){b[TEXT+i*2]=i%94+33;b[TEXT+i*2+1]=i%3?0:0x21;b[TEXT+0x2000+i*2]=(i^mode)|1;}
 for(let i=GRAPHICS;i<b.length;i++)b[i]=(i^i>>>5^i>>>16^mode)&255;
 return b;
}
const outputBuffers=cores.map(()=>new Uint8Array(1024000));
function mutate(b,index){
 const start=new DataView(b.buffer).getUint32(56,true),page=b[6];
 const address=b[5]&32?(page*0x40000+start*8+index)&0x7ffff
  :page*(b[4]?4:3)*0x8000+((start*2+index)&0x7fff);
 b[GRAPHICS+address]^=1;
}
function draw(c,b){const pixels=outputBuffers[cores.indexOf(c)],started=performance.now(),p=c.input_buffer(b.length)>>>0;new Uint8Array(c.memory.buffer,p,b.length).set(b);const out=c.video_render(b.length)>>>0;if(!out)throw Error('invalid snapshot');pixels.set(new Uint8Array(c.memory.buffer,out,1024000));return {ms:performance.now()-started,pixels};}
function equal(a,b){const x=new Uint32Array(a.buffer,a.byteOffset,a.byteLength/4),y=new Uint32Array(b.buffer,b.byteOffset,b.byteLength/4);for(let i=0;i<x.length;i++)if(x[i]!==y[i])throw Error('pixel mismatch '+i);}
let checks=0;
for(const profile of [0,1])for(let mode=0;mode<32;mode++){const b=snapshot(profile,mode);for(const frame of [0,6,8]){new DataView(b.buffer).setBigUint64(8,BigInt(frame)*133333n,true);equal(draw(cores[0],b).pixels,draw(cores[1],b).pixels);checks++;}}
const rows=[];
for(const [name,profile,mode,changing] of [['vm-planar',0,16,true],['ap-planar',1,16,true],['ap-planar40',1,18,true],['ap-static',1,16,false],['ap-pegc',1,7,true]]){
 const b=snapshot(profile,mode);
 for(let i=0;i<256;i++){if(changing)mutate(b,i);for(const c of cores)draw(c,b);}
 for(let repeat=0;repeat<3;repeat++){const times=[0,0],frames=changing?144:1440;for(let n=0;n<frames;n++){if(changing)mutate(b,n);const outputs=[];for(const k of (repeat+n)%2?[1,0]:[0,1]){const r=draw(cores[k],b);times[k]+=r.ms;outputs[k]=r.pixels;}equal(...outputs);checks++;}rows.push({name,repeat,frames,times});postMessage({progress:name,repeat,times});}
}
postMessage({rows,checks});
`;
assets.set('bench.js',Buffer.from(worker));
const server=createServer((req,res)=>{const name=new URL(req.url,'http://localhost').pathname.slice(1);if(!name){res.writeHead(200,{'Content-Type':'text/html'}).end('<!doctype html><title>Owned renderer comparison</title>');return;}if(name==='favicon.ico'){res.writeHead(204).end();return;}const b=assets.get(name);if(!b){res.writeHead(404).end();return;}res.writeHead(200,{'Content-Type':name.endsWith('.wasm')?'application/wasm':'text/javascript','Cache-Control':'no-store'}).end(b);});
const sha=b=>createHash('sha256').update(b).digest('hex');let browser;
try{await new Promise((r,j)=>{server.once('error',j);server.listen(0,'127.0.0.1',r);});browser=await chromium.launch({channel:'chrome',headless:true});const page=await browser.newPage();page.on('console',m=>console.log(m.text()));await page.goto('http://127.0.0.1:'+server.address().port+'/');const result=await page.evaluate(()=>new Promise((resolve,reject)=>{const w=new Worker('/bench.js',{type:'module'});w.onerror=e=>reject(Error(e.message));w.onmessage=e=>{if(e.data.rows){w.terminate();resolve(e.data);}else console.log(JSON.stringify(e.data));};}));await writeFile(opts.output,JSON.stringify({context:{date:new Date().toISOString(),browser:browser.version(),node:process.version,os:os.release(),arch:os.arch(),cpu:os.cpus()[0].model,hashes:Object.fromEntries([...assets].map(([k,v])=>[k,sha(v)])),harnessSha256:sha(await readFile(new URL(import.meta.url)))},...result},null,2),{flag:'wx'});}finally{await browser?.close();server.closeAllConnections();server.close();}
