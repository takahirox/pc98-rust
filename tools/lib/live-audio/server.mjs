import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {readFile,writeFile,mkdtemp,rm} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
const opts=Object.fromEntries(process.argv.slice(2).map(x=>x.slice(2).split('=')));
const {output,executable}=opts;
if (!['0','1'].includes(opts.reference) || !output?.startsWith('/out/') || !executable?.startsWith('/')) throw Error('Use the live audio host runner');
if (!['60','100'].includes(opts.reserve)) throw Error('Unsupported live audio reserve');
const dir=await mkdtemp(join(dirname(output),'live-chrome-'));
const assets=new Map();
for(const [n,file] of [
  ...['live-worker.mjs','live-page.mjs'].map(n=>[n,new URL(n.replace('live-',''),import.meta.url)]),
  ...['core.wasm','presenter.js','block-jit.js','block-compiler.js','block-linker.js','jit.js','audio-worklet.js'].map(n=>[n,'/runtime/'+n]),
  ...((opts.reference==='1')?['np2.mjs','np2.wasm','FONT.ROM','np2kai.cfg'].map(n=>[n,'/reference/'+n]):[]),
  ['game.hdi','/private/game.hdi'],
])assets.set(n,await readFile(file));
assets.set('',Buffer.from('<!doctype html><meta charset=utf-8><style>[hidden]{display:none!important}body{margin:0}#surface{width:675px;height:506px;position:relative}canvas{width:100%;height:100%;position:absolute;inset:0}#surface.crt #raw{visibility:hidden}</style><div id="surface"><canvas id="raw" width="640" height="400"></canvas><canvas id="crt" hidden></canvas></div><script type="module" src="live-page.mjs"></script>'));
const digest=data=>createHash('sha256').update(data).digest('hex');
const assetsHash=Object.fromEntries([...assets].map(([n,b])=>[n,digest(b)]));
const cpuMax=(await readFile('/sys/fs/cgroup/cpu.max','utf8')).trim();
let complete;
const done=new Promise(resolve=>{complete=resolve;});
const server=createServer(async (req,res)=>{
  const name=new URL(req.url,'http://localhost').pathname.slice(1);
  if(name==='result'&&req.method==='POST'){
    let body='';for await(const b of req){body+=b;if(body.length>500000){res.writeHead(413).end();return;}}
    let report;
    try { report=JSON.parse(body); }
    catch { res.writeHead(400).end(); return; }
    res.end('ok');complete(report);return;
  }
  if(!assets.has(name)){res.writeHead(404).end();return;}
  const type=name===''?'text/html':name.endsWith('.wasm')?'application/wasm':/\.m?js$/.test(name)?'text/javascript':'application/octet-stream';
  res.writeHead(200,{'Content-Type':type,'Cache-Control':'no-store'}).end(assets.get(name));
});
let child,stderr='';
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(0,'127.0.0.1',resolve);});
  child=spawn(executable,['--headless','--no-sandbox','--user-data-dir='+dir,'--no-first-run','--no-default-browser-check','--autoplay-policy=no-user-gesture-required',`http://127.0.0.1:${server.address().port}/?reference=${opts.reference}&reserve=${opts.reserve}`],{stdio:['ignore','ignore','pipe']});
  child.stderr.on('data',bytes=>{stderr=(stderr+bytes).slice(-4000);});
  const report=await Promise.race([done,new Promise((_,reject)=>child.once('error',reject).once('exit',code=>reject(Error('Browser exited '+code)))),
    new Promise((_,reject)=>setTimeout(()=>reject(Error('Live playback deadline')),210000).unref())]);
  await writeFile(output,JSON.stringify({assetsHash,cpuMax,...report},null,2),{flag:'wx'});
  console.log(JSON.stringify({error:report.error,samples:report.samples?.length}));
  if(report.error)process.exitCode=1;
}catch(error){console.error(String(error));process.exitCode=1;}
finally{
 if(child&&child.exitCode===null){
  const exited=new Promise(resolve=>child.once('exit',resolve));
  child.kill('SIGTERM');
  let timer;
  await Promise.race([exited,new Promise(resolve=>{timer=setTimeout(resolve,2000);})]).finally(()=>clearTimeout(timer));
 }
 server.closeAllConnections();server.close();await rm(dir,{recursive:true,force:true}).catch(()=>{});
 if(process.exitCode)console.error(stderr);
}
