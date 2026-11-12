// Opt-in normal-browser reproduction. No automation protocol or guest fast-forward.
// See docs/doukyuusei2-reproduction.md. Game media and raw reports stay local.
import {createServer} from 'node:http';
import {spawn} from 'node:child_process';
import {access,readFile,writeFile,mkdtemp,realpath} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import os from 'node:os';

const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
options.jit??='0';
const engine=options.browser, clicks=Number(options.clicks??48);
for(const name of ['headless','noSandbox','samplePatches'])if(options[name]!==undefined&&!['0','1'].includes(options[name]))throw Error('Expected --'+name+'=0|1');
const headless=options.headless==='1',noSandbox=options.noSandbox==='1',samplePatches=options.samplePatches!=='0';
if(engine==='firefox'&&(headless||noSandbox))throw Error('Headless/container flags apply only to Chromium controls');
if(options.jit!==undefined&&!['0','1'].includes(options.jit))throw Error('Expected --jit=0|1');
if(!['chrome','firefox'].includes(engine) || !Number.isInteger(clicks) || clicks<1 || clicks>240)
  throw Error('Expected --browser=chrome|firefox and --clicks=1..240');
const root=await realpath('.');
for(const name of ['media','output','executable']) {
  if(!options[name] || !path.isAbsolute(options[name])) throw Error(`--${name} must be an absolute path`);
}
for(const name of ['media','output']) {
  const p=await realpath(name==='output'?path.dirname(options[name]):options[name]);
  if(p===root || p.startsWith(root+path.sep)) throw Error(`--${name} must stay outside the checkout`);
}
options.executable=await realpath(options.executable);
if(engine==='firefox') {
  const folder=path.dirname(options.executable);
  for(const file of [path.join(folder,'playwright.cfg'),path.resolve(folder,'../Resources/playwright.cfg')]) {
    const patched=await access(file).then(()=>true,()=>false);
    if(patched) throw Error('Use stock Mozilla Firefox for this control; a Playwright-patched build is not accepted, even without a connected driver.');
  }
}
await access(options.output).then(()=>{throw Error('Output already exists');},()=>{});
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const names=['app.js','emulator-worker.js','audio-worklet.js','render-worker.js','core.wasm','jit.js'];
const assets=new Map(await Promise.all(names.map(async name=>[name,await readFile(path.join(root,'web',name))])));
const sourceHashes=Object.fromEntries([...assets].map(([name,body])=>[name,sha(body)]));
// These bounded input/audio getters exist only in the served local control.
// The emulator Worker, renderer, worklet and Wasm bytes remain unchanged.
assets.set('app.js',Buffer.from(assets.get('app.js').toString()+`
window.reproductionMouse=(...args)=>core.mouse(...args);
window.reproductionAudio=()=>{
  const modern=audio?.playbackStats, legacy=audio?.playoutStats;
  if(modern)return {supported:true,api:'playbackStats',totalSeconds:modern.totalDuration,
    underrunSeconds:modern.underrunDuration,underrunEvents:modern.underrunEvents,
    averageLatencySeconds:modern.averageLatency,maximumLatencySeconds:modern.maximumLatency};
  if(legacy)return {supported:true,api:'playoutStats',totalSeconds:legacy.totalFramesDuration/1000,
    underrunSeconds:legacy.fallbackFramesDuration/1000,underrunEvents:legacy.fallbackFramesEvents,
    averageLatencySeconds:legacy.averageLatency/1000,maximumLatencySeconds:legacy.maximumLatency/1000};
  return {supported:false};
};
`));
const servedHashes=Object.fromEntries([...assets].map(([name,body])=>[name,sha(body)]));
const harnessHash=sha(await readFile(new URL(import.meta.url)));
const counts=Object.fromEntries(names.map(name=>[name,0]));
const runner=`
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const began=performance.now(), rows=[], events=[];let timer,pending=false,failure,phase='startup';
async function sample(){
  if(pending)return;pending=true;
  try{
    const s=await pc98.stats(),context=document.querySelector('#screen').getContext('2d'),patches=[];
    if(${samplePatches})for(const y of [30,140,250])for(const x of [70,310,550]){
      let hash=2166136261;for(const byte of context.getImageData(x,y,16,16).data)hash=Math.imul(hash^byte,16777619)>>>0;
      patches.push(hash);
    }
    rows.push({wallMs:performance.now()-began,phase,guestSeconds:s.guestSeconds,underflows:s.underflows,
      producedSamples:s.producedSamples,consumedSamples:s.consumedSamples,queuedSamples:s.queuedSamples,
      audioEnergy:s.audioEnergy,audioState:s.audioState,running:s.running,audioEpoch:s.audioEpoch,
      overflows:s.overflows,droppedSamples:s.droppedSamples,sequenceErrors:s.sequenceErrors,checksumErrors:s.checksumErrors,
      frames:s.presentationStats.frames,guestSlices:s.videoStats.guestSlices,guestSliceMs:s.videoStats.guestSliceMs,
      hddReads:s.hddReads,fmKeyEdges:s.fmKeyEdges,soundIrqEdges:s.soundIrqEdges,
      cpuCycles:s.cpuCycles,completedInstructions:s.completedInstructions,repElements:s.repElements,
      videoMode:s.videoMode,rendererFailure:s.rendererFailure,error:s.error,hidden:document.hidden,
      underflowRun:s.underflowRun,maxUnderflowRun:s.maxUnderflowRun,outstandingSamples:s.outstandingSamples,
      scheduledSamples:s.scheduledSamples,deviceSampleRate:s.deviceSampleRate,
      sliceHistogram:s.sliceHistogram,videoStats:s.videoStats,renderStats:s.renderStats,presentationStats:s.presentationStats,
      output:reproductionAudio(),patches,jit:s.jit});
    if(s.error || !s.running || s.audioState!=='running')throw Error('Playback stopped or faulted');
  }finally{pending=false;}
}
async function wait(check){for(let n=0;!await check();n++){
  if(failure)throw failure;
  const error=document.querySelector('#error');if(error&&!error.hidden&&error.textContent)throw Error('UI: '+error.textContent);
  if(n>=1800)throw Error('Setup deadline in '+phase);await sleep(100);
}}
async function key(code){
  const screen=document.querySelector('#screen');screen.dispatchEvent(new KeyboardEvent('keydown',{code,bubbles:true}));
  await sleep(120);screen.dispatchEvent(new KeyboardEvent('keyup',{code,bubbles:true}));await sleep(80);
}
const scan=c=>c===' '?'Space':c==='2'?'Digit2':'Key'+c.toUpperCase();
async function command(text){for(const c of text)await key(scan(c));await key('Enter');await sleep(600);}
async function mouse(dx,dy,buttons=0,ms=120){await reproductionMouse(dx,dy,buttons);await sleep(ms);}
function mark(name){phase=name;events.push({name,wallMs:performance.now()-began});
  fetch('/phase',{method:'POST',body:name}).catch(()=>{});}
try{
  await wait(()=>window.pc98&&window.reproductionMouse&&!document.querySelector('#file').disabled);
  document.querySelector('#drive').value='2';
  const transfer=new DataTransfer();transfer.items.add(new File([await(await fetch('/private-test-media')).arrayBuffer()],'installed.hdi'));
  document.querySelector('#file').files=transfer.files;document.querySelector('#file').dispatchEvent(new Event('change',{bubbles:true}));
  await wait(()=>!document.querySelector('#file').disabled);document.querySelector('#reset').click();
  await wait(()=>!document.querySelector('#reset').disabled);document.querySelector('#sound').click();
  await wait(async()=>(await pc98.stats()).audioState==='running');
  mark('audio-boot');await sample();timer=setInterval(()=>sample().catch(error=>{failure=error;}),250);
  await wait(async()=>(await pc98.stats()).guestSeconds>=12);
  mark('launch');await command('cd nanpa2');await command('nanpa2');
  await wait(async()=>(await pc98.stats()).hddReads>=649);await sleep(1500);await key('Enter');
  await wait(async()=>(await pc98.stats()).hddReads>=825);await sleep(3000);
  // Guest PPI input with real wall time between movements; never write game RAM.
  for(let n=0;n<8;n++)await mouse(-100,-100);for(let n=0;n<5;n++)await mouse(100,50);
  await mouse(20,0,1,300);await mouse(0,0,0,2000);mark('prologue');await mouse(-20,100);
  for(let n=0;n<48;n++){
    if(failure)throw failure;
    events.push({name:'click',index:n,wallMs:performance.now()-began});
    await mouse(0,0,1,100);await mouse(0,0,0,500);
  }
  // Select Listen, then its first topic, to exercise the flashback CG and return.
  mark('classroom-choice');await mouse(0,0,2);await mouse(0,0,0);
  for(let n=0;n<3;n++)await mouse(-100,0);await mouse(-50,20);
  await mouse(0,0,1,100);await mouse(0,0,0,500);await mouse(0,-25);
  await mouse(0,0,1,100);await mouse(0,0,0,500);mark('flashback');
  for(let n=0;n<${clicks};n++){
    if(failure)throw failure;
    events.push({name:'flashback-click',index:n,wallMs:performance.now()-began});
    await mouse(0,0,1,100);await mouse(0,0,0,500);
  }
  await sample();clearInterval(timer);await wait(()=>!pending);if(failure)throw failure;
  await fetch('/result',{method:'POST',body:JSON.stringify({userAgent:navigator.userAgent,events,rows})});
}catch(error){
  clearInterval(timer);await fetch('/result',{method:'POST',body:JSON.stringify({error:String(error),phase,events,rows})});
}
`;
const profile=await mkdtemp(path.join(path.dirname(options.output),'pc98-reproduction-'));
if(engine==='firefox') await writeFile(path.join(profile,'user.js'),[
  'user_pref("browser.shell.checkDefaultBrowser", false);',
  'user_pref("browser.startup.homepage_override.mstone", "ignore");',
  'user_pref("browser.aboutwelcome.enabled", false);',
  'user_pref("startup.homepage_welcome_url", "about:blank");',
  'user_pref("startup.homepage_welcome_url.additional", "");',
  'user_pref("media.autoplay.default", 0);','user_pref("media.autoplay.blocking_policy", 0);','',
].join('\n'));
let finish,child;
const cgroups=async()=>Object.fromEntries(await Promise.all(['cpu.max','cpu.stat','cpuset.cpus.effective'].map(async name=>[name,await readFile('/sys/fs/cgroup/'+name,'utf8').catch(()=>null)])));
const completed=new Promise(resolve=>{finish=resolve;});
const server=createServer(async(request,response)=>{
  try{
    const name=decodeURIComponent(new URL(request.url,'http://localhost').pathname).slice(1)||'index.html';
    if(name==='phase' && request.method==='POST'){
      let phase='';for await(const chunk of request){phase+=chunk;if(phase.length>40)throw Error('Invalid phase');}
      if(['audio-boot','launch','prologue','classroom-choice','flashback'].includes(phase))console.log(JSON.stringify({phase}));
      response.end('ok');return;
    }
    if(name==='result' && request.method==='POST'){
      let body='';for await(const chunk of request){body+=chunk;if(body.length>4*1024*1024)throw Error('Report exceeds bound');}
      response.end('ok');finish(JSON.parse(body));return;
    }
    if(name==='private-test-media'){
      response.writeHead(200,{'Content-Type':'application/octet-stream','Cache-Control':'no-store'}).end(await readFile(options.media));return;
    }
    if(name==='reproduction-runner.js'){
      response.writeHead(200,{'Content-Type':'text/javascript','Cache-Control':'no-store'}).end(runner);return;
    }
    const location=path.resolve(root,'web',name);
    if(!location.startsWith(path.join(root,'web')+path.sep))throw Error('Outside publication tree');
    let body=assets.has(name)?assets.get(name):await readFile(location);
    if(assets.has(name))counts[name]++;
    if(name==='index.html')body=Buffer.from(body.toString().replace('</body>','<script type="module" src="reproduction-runner.js"></script></body>'));
    const type={'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html','.json':'application/json','.css':'text/css'}[path.extname(name)]??'application/octet-stream';
    response.writeHead(200,{'Content-Type':type,'Cache-Control':'no-store'}).end(body);
  }catch{response.writeHead(404).end();}
});
try{
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(8079,'127.0.0.1',resolve);});
  const url='http://127.0.0.1:8079/?diagnostics&jit='+(options.jit==='0'?'0':'1');
  const args=engine==='firefox'?['-no-remote','-profile',profile,url]:
    [...(headless?['--headless']:[]),...(noSandbox?['--no-sandbox']:[]),'--user-data-dir='+profile,'--no-first-run','--no-default-browser-check','--autoplay-policy=no-user-gesture-required',url];
  const cgroupBefore=await cgroups();
  child=spawn(options.executable,args,{stdio:['ignore','ignore','pipe']});
  let stderr='';child.stderr.on('data',chunk=>{stderr=(stderr+chunk).slice(-1500);});
  const result=await Promise.race([completed,
    new Promise((_,reject)=>child.once('error',reject).once('exit',code=>reject(Error('Browser exited '+code)))),
    new Promise((_,reject)=>setTimeout(()=>reject(Error('Reproduction deadline; '+stderr)),240000+clicks*600).unref()),
  ]);
  const missing=names.filter(name=>!counts[name] && !(name==='jit.js'&&options.jit==='0'));
  if(missing.length && !result.error)result.error='Selected runtime assets were not served: '+missing.join(', ');
  await writeFile(options.output,JSON.stringify({context:{date:new Date().toISOString(),engine,
    automationProtocol:false,manualGuestAdvance:false,headless,noSandbox,samplePatches,automaticAudioActivation:true,
    os:os.type()+' '+os.release()+' '+os.arch(),cpu:os.cpus()[0].model,node:process.version,
    sourceHashes,servedHashes,harnessHash,mediaSha256:sha(await readFile(options.media)),counts,clicks,jit:options.jit!=='0',
    cgroupBefore,cgroupAfter:await cgroups()},...result},null,2)+'\n',{flag:'wx'});
  const a=result.rows[0],b=result.rows.at(-1);
  console.log(JSON.stringify({error:result.error,rows:result.rows.length,
    guestSecondsPerWallSecond:a?(b.guestSeconds-a.guestSeconds)*1000/(b.wallMs-a.wallMs):null,
    missingDeviceSamples:a?b.underflows-a.underflows:null,output:b?.output,hddReads:b?.hddReads,fmKeyEdges:b?.fmKeyEdges}));
  if(result.error)process.exitCode=1;
}finally{child?.kill('SIGTERM');server.closeAllConnections();server.close();}
