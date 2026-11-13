// Opt-in, network-isolated container controls. Private media never enters web/.
import {spawn,execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash,randomUUID} from 'node:crypto';
import {access,copyFile,mkdir,mkdtemp,readFile,readdir,realpath,writeFile} from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {budgetOptions,image,executable,playbackSummary} from './lib/cpu-budget.mjs';

const execute=promisify(execFile),root=await realpath('.');
const harnessHash=createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex');
const options=Object.fromEntries(process.argv.slice(2).map(v=>v.replace(/^--/,'').split('=')));
const budgets=(options.budgets??'unlimited,1,0.5,0.25').split(',').map(budgetOptions);
// Git can retain prunable records after temporary worktrees disappear. Those
// paths cannot contain media; existing checkouts still require canonical checks.
const worktrees=(await Promise.all((await execute('git',['worktree','list','--porcelain','-z'],{cwd:root})).stdout.split('\0').filter(s=>s.startsWith('worktree ')).map(s=>realpath(s.slice(9)).catch(error=>{
 if(error.code==='ENOENT')return null;throw error;
})))).filter(Boolean);
const repeat=Number(options.repeat??1),clicks=Number(options.clicks??24);
const samplePatches=options.samplePatches??'0';
if(!['0','1'].includes(samplePatches))throw Error('Expected --samplePatches=0|1');
if(!Number.isInteger(repeat)||repeat<1||repeat>3||!Number.isInteger(clicks)||clicks<12||clicks>120)throw Error('Expected --repeat=1..3 and --clicks=12..120');
if(budgets[0].label!=='unlimited'||new Set(budgets.map(b=>b.label)).size!==budgets.length)throw Error('Start with one unrestricted control; budgets must be unique');
for(const name of ['media','output']){
 if(!options[name]||!path.isAbsolute(options[name])||/[,\r\n]/.test(options[name]))throw Error('--'+name+' must be an absolute Docker-bind-compatible path');
 const location=await realpath(name==='output'?path.dirname(options[name]):options[name]);
 if(/[,\r\n]/.test(location))throw Error('Resolved bind paths must not contain commas or newlines');
 if([root,...worktrees].some(tree=>location===tree||location.startsWith(tree+path.sep)))throw Error('--'+name+' must stay outside every checkout');
}
await access(options.output).then(()=>{throw Error('Output exists');},()=>{});
await execute(process.execPath,['tools/check_site.mjs'],{cwd:root});
const docker=async args=>(await execute('docker',args,{timeout:30000,maxBuffer:1024*1024})).stdout.trim();
const identity=JSON.parse(await docker(['image','inspect',image,'--format','{"Id":{{json .Id}},"Architecture":{{json .Architecture}}}']));
if(!/^sha256:[a-f0-9]{64}$/.test(identity.Id))throw Error('Invalid cached image identity');
const directory=await mkdtemp(path.join(path.dirname(options.output),'pc98-cpu-budget-'));
const frozen=path.join(directory,'frozen');
await mkdir(path.join(frozen,'tools/lib'),{recursive:true});await mkdir(path.join(frozen,'web'),{recursive:true});
const toolHashes={};
for(const name of ['reproduce_doukyuusei2.mjs','calibrate_cpu_budget.mjs','lib/cpu-budget.mjs']){
 await copyFile(path.join(root,'tools',name),path.join(frozen,'tools',name));
 toolHashes[name]=createHash('sha256').update(await readFile(path.join(frozen,'tools',name))).digest('hex');
}
const hashes={};
async function freezeWeb(relative=''){
 for(const entry of await readdir(path.join(root,'web',relative),{withFileTypes:true})){
  const name=path.join(relative,entry.name);
  if(entry.isDirectory()){
   if(!['media','freedos'].includes(name))throw Error('Unexpected web directory: '+name);
   await mkdir(path.join(frozen,'web',name));await freezeWeb(name);
  }else{
   if(!entry.isFile())throw Error('Non-regular web asset: '+name);
   const bytes=await readFile(path.join(root,'web',name));hashes[name]=createHash('sha256').update(bytes).digest('hex');
   await writeFile(path.join(frozen,'web',name),bytes,{flag:'wx'});
  }
 }
}
await freezeWeb();
const media=await realpath(options.media),mediaHash=createHash('sha256').update(await readFile(media)).digest('hex');
const rows=[];let owned,failure,written=false;
const stop=async()=>{
 if(!owned)return;
 const id=owned;
 try{await docker(['rm','--force',id]);owned=null;}
 catch(error){if(/No such container/.test(error.stderr??''))owned=null;else{failure??='Cleanup failed for owned container '+id;throw error;}}
};
const save=async()=>{
 if(written)return;written=true;
 await writeFile(options.output,JSON.stringify({context:{date:new Date().toISOString(),image,imageId:identity.Id,architecture:identity.Architecture,
  os:os.type()+' '+os.release()+' '+os.arch(),sourceHashes:hashes,toolHashes,
  harnessHash,ownedContainer:owned??null,
  mediaHash,repeat,clicks,privateDirectory:directory,
  scope:'CPU quota for the entire owned browser container; not slow-core, cache or GPU equivalence',automationProtocol:false,samplePatches:samplePatches==='1'},error:failure??null,rows},null,2)+'\n',{flag:'wx'});
};
for(const signal of ['SIGINT','SIGTERM'])process.once(signal,()=>{failure='Interrupted by '+signal;void stop().catch(error=>{failure+='; '+String(error);}).then(save).finally(()=>process.exit(1));});
try{
 for(let run=0;run<repeat;run++)for(const budget of run%2?[budgets[0],...budgets.slice(1).reverse()]:budgets){
  const prefix='run-'+run+'-cpu-'+budget.label;
  const calibration=prefix+'-calibration.json',game=prefix+'-game.json';
  const command=`node tools/calibrate_cpu_budget.mjs --executable=${executable} --output=/out/${calibration} && exec node tools/reproduce_doukyuusei2.mjs --browser=chrome --jit=1 --headless=1 --noSandbox=1 --samplePatches=${samplePatches} --executable=${executable} --media=/private/installed.hdi --output=/out/${game} --clicks=${clicks}`;
  const args=['create','--init','--network=none','--memory=2g','--shm-size=256m','--cpuset-cpus=0,1',
   '--cpu-period='+budget.period,'--cpu-quota='+budget.quota,'--name','pc98-cpu-'+randomUUID(),
   '--label','pc98.cpu-budget=measurement','--mount',`type=bind,src=${path.join(frozen,'tools')},dst=/repo/tools,readonly`,
   '--mount',`type=bind,src=${path.join(frozen,'web')},dst=/repo/web,readonly`,
   '--mount',`type=bind,src=${media},dst=/private/installed.hdi,readonly`,
   '--mount',`type=bind,src=${directory},dst=/out`,'--workdir=/repo',identity.Id,'sh','-c',command];
  owned=await docker(args);if(!/^[a-f0-9]{64}$/.test(owned))throw Error('Invalid owned container identity');
  console.log(JSON.stringify({run,budget:budget.label,phase:'start'}));
  const configured=JSON.parse(await docker(['inspect',owned,'--format','{"CpuPeriod":{{.HostConfig.CpuPeriod}},"CpuQuota":{{.HostConfig.CpuQuota}},"CpusetCpus":{{json .HostConfig.CpusetCpus}},"Memory":{{.HostConfig.Memory}}}']));
  const child=spawn('docker',['start','--attach',owned],{stdio:['ignore','pipe','pipe']});
  let stderr='';child.stdout.on('data',data=>process.stdout.write(data));child.stderr.on('data',data=>{stderr=(stderr+data).slice(-3000);});
  let timer;
  const attachCode=await Promise.race([new Promise((resolve,reject)=>child.once('error',reject).once('exit',resolve)),
   new Promise((_,reject)=>{timer=setTimeout(()=>reject(Error('Bounded container deadline')),420000+clicks*600);})]).finally(()=>clearTimeout(timer));
  const state=JSON.parse(await docker(['inspect',owned,'--format','{"ExitCode":{{.State.ExitCode}},"OOMKilled":{{.State.OOMKilled}}}'])),code=state.ExitCode;
  const measured=JSON.parse(await readFile(path.join(directory,calibration),'utf8'));
  const playback=JSON.parse(await readFile(path.join(directory,game),'utf8'));
  const cpuMax=measured.context.cgroupBefore['cpu.max']?.trim();
  if(cpuMax!==(budget.quota<0?'max':String(budget.quota))+' '+budget.period)throw Error('Observed quota differs from requested quota: '+cpuMax);
  for(const value of [measured.context.cgroupAfter,playback.context.cgroupBefore,playback.context.cgroupAfter])if(value['cpu.max']?.trim()!==cpuMax)throw Error('CPU control changed during the run');
  if(playback.context.mediaSha256!==mediaHash)throw Error('Media identity changed');
  for(const [name,hash]of Object.entries(playback.context.sourceHashes))if(hashes[name]!==hash)throw Error('Runtime identity changed: '+name);
  const phases={};let validityError=playback.error??(code||attachCode?'Container exited with an error':null);
  for(const phase of ['prologue','flashback']){
   const samples=playback.rows.filter(r=>r.phase===phase);
   try{phases[phase]=playbackSummary(samples);if(phases[phase].wallMs<5000)throw Error('Scene window too short');}
   catch(error){phases[phase]=null;validityError??=String(error);}
  }
  const reference=rows.find(r=>r.run===run&&r.budget==='unlimited')?.calibration;
  const slowdown=Object.fromEntries(Object.entries(measured.summary).map(([actor,value])=>[actor,reference?value.medianMs/reference[actor].medianMs:1]));
  const expectedSingleThreadSlowdown=budget.quota<0?1:Math.max(1,budget.period/budget.quota);
  const workersLimited=expectedSingleThreadSlowdown===1||['workerA','workerB'].every(actor=>slowdown[actor]>1.2);
  if(!workersLimited)validityError??='Dedicated Workers did not materially slow under the requested sub-core quota';
  const row={run,budget:budget.label,period:budget.period,quota:budget.quota,
   configured:{period:configured.CpuPeriod,quota:configured.CpuQuota,cpuset:configured.CpusetCpus,memory:configured.Memory},
   cpuMax,exitCode:code,error:validityError,valid:!validityError,oomKilled:state.OOMKilled,stderr:code?stderr:undefined,
   calibration:measured.summary,slowdown,expectedSingleThreadSlowdown,workersLimited,phases,files:{calibration,game}};
  rows.push(row);console.log(JSON.stringify({run,budget:budget.label,slowdown,phases,error:row.error}));
  await stop();
 }
}catch(error){failure=String(error);throw error;}
finally{try{await stop();}finally{await save();}}
