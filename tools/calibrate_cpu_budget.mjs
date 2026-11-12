// Standalone owned-browser Wasm calibration: no CDP, debugger or profiler.
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {mkdtemp,readFile,realpath,writeFile} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {calibrationModule,expectedCalibration,percentile} from './lib/cpu-budget.mjs';

const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
const count=Number(options.iterations??30000000),root=await realpath('.');
if(!Number.isInteger(count)||count<1000000||count>100000000)throw Error('Expected --iterations=1000000..100000000');
if(!options.output||!path.isAbsolute(options.output))throw Error('--output must be absolute');
const folder=await realpath(path.dirname(options.output));
if(folder===root||folder.startsWith(root+path.sep))throw Error('Keep reports outside the checkout');
const browser=options.executable;
if(!browser||!path.isAbsolute(browser))throw Error('--executable must be absolute');
const bytes=Array.from(calibrationModule()),expected=expectedCalibration(count);
const script=`
const bytes=new Uint8Array(${JSON.stringify(bytes)}),count=${count},expected=${expected};
const run=(await WebAssembly.instantiate(bytes,{})).instance.exports.run;
const workerSource=\`let run;onmessage=async({data})=>{if(data.bytes){run=(await WebAssembly.instantiate(data.bytes,{})).instance.exports.run;for(let n=0;n<10;n++)run(1000000);postMessage({ready:true});}else{const start=performance.now(),result=run(data.count)>>>0;postMessage({ms:performance.now()-start,result});}};\`;
const url=URL.createObjectURL(new Blob([workerSource],{type:'text/javascript'}));
const workers=[new Worker(url),new Worker(url)],rows=[];
try{
 await Promise.all(workers.map(w=>new Promise(resolve=>{w.onmessage=resolve;w.postMessage({bytes});})));
 for(let n=0;n<10;n++)run(1000000);
 for(let round=0;round<9;round++)for(const actor of [0,1,2].map(n=>(n+round)%3)){
  let value;
  if(actor===0){const start=performance.now(),result=run(count)>>>0;value={ms:performance.now()-start,result};}
  else value=await new Promise(resolve=>{const w=workers[actor-1];w.onmessage=({data})=>resolve(data);w.postMessage({count});});
  if(value.result!==expected)throw Error('Calibration result mismatch');
  rows.push({actor:['page','workerA','workerB'][actor],...value});
 }
 await fetch('/result',{method:'POST',body:JSON.stringify({userAgent:navigator.userAgent,rows})});
}catch(error){await fetch('/result',{method:'POST',body:JSON.stringify({error:String(error),rows})});}
finally{workers.forEach(w=>w.terminate());URL.revokeObjectURL(url);}
`;
let finish,child;
const completed=new Promise(resolve=>{finish=resolve;});
const server=createServer(async(req,res)=>{
 if(req.url==='/result'&&req.method==='POST'){
  let body='';for await(const chunk of req){body+=chunk;if(body.length>100000){res.writeHead(413).end();return;}}
  res.end('ok');finish(JSON.parse(body));
 }else if(req.url==='/'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><script type="module">'+script+'</script>');}
 else res.writeHead(404).end();
});
const cgroups=async()=>Object.fromEntries(await Promise.all(['cpu.max','cpu.stat','cpuset.cpus.effective'].map(async name=>[name,await readFile('/sys/fs/cgroup/'+name,'utf8').catch(()=>null)])));
const profile=await mkdtemp(path.join(os.tmpdir(),'pc98-cpu-calibration-'));
try{
 await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(8078,'127.0.0.1',resolve);});
 const before=await cgroups();
 child=spawn(browser,['--headless','--no-sandbox','--user-data-dir='+profile,'--no-first-run','--no-default-browser-check','http://127.0.0.1:8078/'],{stdio:'ignore'});
 const result=await Promise.race([completed,new Promise((_,reject)=>child.once('error',reject).once('exit',code=>reject(Error('Browser exit '+code)))),
  new Promise((_,reject)=>setTimeout(()=>reject(Error('Calibration deadline')),90000).unref())]);
 const summary=Object.fromEntries(['page','workerA','workerB'].map(actor=>{const times=result.rows.filter(r=>r.actor===actor).map(r=>r.ms);return [actor,{medianMs:percentile(times,0.5),p95Ms:percentile(times,0.95),samples:times.length}];}));
 if(result.error||result.rows.length!==27)throw Error(result.error??'Incomplete calibration');
 await writeFile(options.output,JSON.stringify({context:{date:new Date().toISOString(),iterations:count,expected,automationProtocol:false,
  headless:true,sandbox:false,cgroupBefore:before,cgroupAfter:await cgroups()},...result,summary},null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({calibration:summary}));
}finally{child?.kill('SIGTERM');server.closeAllConnections();server.close();}
