// Opt-in whole-browser control with the same audio sink and CRT presenter.
// Keep the private disk, font and all raw reports outside version control.
import {execFile,spawn} from 'node:child_process';
import {promisify} from 'node:util';
import {randomUUID} from 'node:crypto';
import {copyFile,mkdir,readFile,realpath,writeFile} from 'node:fs/promises';
import {isAbsolute,join,dirname,sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import {budgetOptions,image,executable} from './lib/cpu-budget.mjs';

const execute=promisify(execFile);
const root=await realpath(fileURLToPath(new URL('../',import.meta.url)));
const options=Object.fromEntries(process.argv.slice(2).map(value=>value.replace(/^--/,'').split('=')));
const repeat=Number(options.repeat??1);
const budgets=(options.budgets??'unlimited,0.5').split(',').map(budgetOptions);
if(!Number.isInteger(repeat)||repeat<1||repeat>3||budgets[0].label!=='unlimited'||new Set(budgets.map(b=>b.label)).size!==budgets.length)
  throw Error('Use --repeat=1..3 and start --budgets with a single unlimited control');
const worktrees=(await execute('git',['worktree','list','--porcelain','-z'],{cwd:root})).stdout.split('\0')
  .filter(field=>field.startsWith('worktree ')).map(field=>field.slice(9));
const checkouts=(await Promise.all(worktrees.map(tree=>realpath(tree).catch(error=>{
  if(error.code==='ENOENT')return null;
  throw error;
})))).filter(Boolean);
for(const name of ['media','output','reference','font','config']){
  if(!options[name]||!isAbsolute(options[name]))
    throw Error('Required absolute paths: --media=HDI --output=NEW_DIR --reference=NP2KAI_BUILD --font=FONT_ROM --config=NP2KAI_CFG');
  const canonical=await realpath(name==='output'?dirname(options[name]):options[name]);
  if([root,...checkouts].some(tree=>canonical===tree||canonical.startsWith(tree+sep))||/[,\r\n]/.test(canonical))
    throw Error('--'+name+' must stay outside this checkout and be safe for Docker bind mounts');
  if(name!=='output')options[name]=canonical;
}
await mkdir(options.output);
const output=await realpath(options.output);
if(/[,\r\n]/.test(output))throw Error('Output contains an unsupported bind-path character');
const runtime=await realpath(options.runtime??join(root,'web'));
const frozen=join(output,'frozen');
for(const subdir of ['runtime','reference','runner','runner/lib'])await mkdir(join(frozen,subdir),{recursive:true});
for(const name of ['core.wasm','presenter.js','block-jit.js','block-compiler.js','block-linker.js','jit.js','audio-worklet.js'])
  await copyFile(join(runtime,name),join(frozen,'runtime',name));
for(const name of ['np2.mjs','np2.wasm'])await copyFile(join(options.reference,name),join(frozen,'reference',name));
await copyFile(options.font,join(frozen,'reference/FONT.ROM'));
await copyFile(options.config,join(frozen,'reference/np2kai.cfg'));
for(const name of ['worker.mjs','page.mjs','server.mjs'])await copyFile(join(root,'tools/lib/live-audio',name),join(frozen,'runner',name));
for(const name of ['calibrate_cpu_budget.mjs','lib/cpu-budget.mjs'])await copyFile(join(root,'tools',name),join(frozen,'runner',name));

const docker=async args=>(await execute('docker',args,{timeout:30000,maxBuffer:1048576})).stdout.trim();
const identity=JSON.parse(await docker(['image','inspect',image,'--format','{"id":{{json .Id}},"arch":{{json .Architecture}}}']));
if(!/^sha256:[a-f0-9]{64}$/.test(identity.id))throw Error('Invalid cached image identity');
let owned;
const stop=async()=>{
  if(!owned)return;
  await docker(['rm','--force',owned]);
  owned=null;
};
for(const signal of ['SIGINT','SIGTERM'])process.once(signal,()=>{void stop().finally(()=>process.exit(1));});
await writeFile(join(output,'configuration.json'),JSON.stringify({identity,repeat,budgets,automationProtocol:false},null,2));
const calibrationControls=new Map();
for(let run=0;run<repeat;run++)for(const budget of budgets)for(const engine of run%2?['np2kai','pc98-rust']:['pc98-rust','np2kai']){
  const name=`${run}-${budget.label}-${engine}`,directory=join(output,name);
  await mkdir(directory);
  const readOnly=[[join(frozen,'runtime'),'/runtime'],[join(frozen,'reference'),'/reference'],
    [join(frozen,'runner'),'/input'],[options.media,'/private/game.hdi']];
  const script=`node /input/calibrate_cpu_budget.mjs --executable=${executable} --output=/out/calibration.json && exec node /input/server.mjs --executable=${executable} --output=/out/result.json --reference=${engine==='np2kai'?1:0}`;
  owned=await docker(['create','--init','--network=none','--memory=2g','--shm-size=256m','--cpuset-cpus=0,1',
    '--cpu-period='+budget.period,'--cpu-quota='+budget.quota,'--name','pc98-live-'+randomUUID(),
    ...readOnly.flatMap(([src,dst])=>['--mount',`type=bind,src=${src},dst=${dst},readonly`]),
    '--mount',`type=bind,src=${directory},dst=/out`,identity.id,'sh','-c',script]);
  if(!/^[a-f0-9]{64}$/.test(owned))throw Error('Unexpected owned container ID');
  console.log(JSON.stringify({name,phase:'start'}));
  try{
    const child=spawn('docker',['start','--attach',owned],{stdio:'inherit'});
    let timer;
    const code=await Promise.race([
      new Promise((resolve,reject)=>child.once('error',reject).once('exit',resolve)),
      new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Live container deadline')),240000);}),
    ]).finally(()=>clearTimeout(timer));
    if(code!==0)throw Error(name+' did not complete');
    const report=JSON.parse(await readFile(join(directory,'result.json'),'utf8'));
    if(report.cpuMax!==`${budget.quota<0?'max':budget.quota} ${budget.period}`||report.sampleRate!==48000||
      report.reference!==(engine==='np2kai')||report.error)
      throw Error(name+' failed runtime validation');
    const calibration=JSON.parse(await readFile(join(directory,'calibration.json'),'utf8')).summary;
    const key=`${run}-${engine}`;
    if(budget.quota<0)calibrationControls.set(key,calibration);
    const control=calibrationControls.get(key);
    const slowdown=Object.fromEntries(['page','workerA','workerB'].map(actor=>[actor,calibration[actor].medianMs/control[actor].medianMs]));
    if(budget.quota>0&&budget.quota<budget.period&&Math.min(slowdown.workerA,slowdown.workerB)<=1.2)
      throw Error(name+' did not materially slow both dedicated Workers');
    const phases={};
    for(const phase of ['stationary','progressing']){
      const window=report.samples.filter(row=>row.phase===phase),first=window[0],last=window.at(-1);
      const wall=(last?.wallMs-first?.wallMs)/1000;
      if(wall<20||!Number.isFinite(wall)||window.some(row=>!row.crtActive||!row.sink||!Number.isFinite(row.seconds)))
        throw Error(name+' has no valid '+phase+' playback window');
      const missing=last.sink.underflows-first.sink.underflows, pcm=last.sink.scheduledSamples-first.sink.scheduledSamples;
      if(!Number.isFinite(pcm)||pcm<=0||!Number.isFinite(missing)||missing<0||last.sink.audioEnergy<=first.sink.audioEnergy)
        throw Error(name+' did not produce measurable audio in '+phase);
      phases[phase]={wall,guestRatio:(last.seconds-first.seconds)/wall,fps:(last.frames-first.frames)/wall,
        missing,pcmRatio:pcm/48000/wall,callbackRatio:(pcm+missing)/48000/wall,
        dropped:last.dropped-first.dropped,overflows:last.sink.overflows-first.sink.overflows,
        sequenceErrors:last.sink.sequenceErrors-first.sink.sequenceErrors,
        maxGapMs:last.sink.maxUnderflowRun/48,uploads:last.uploads-first.uploads};
    }
    await writeFile(join(directory,'summary.json'),JSON.stringify({slowdown,phases},null,2));
    console.log(JSON.stringify({name,slowdown,phases}));
  }finally{await stop();}
}
