import {test} from 'node:test';
import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import os from 'node:os';
import path from 'node:path';
import {budgetOptions,calibrationModule,expectedCalibration,percentile,playbackSummary} from '../tools/lib/cpu-budget.mjs';

test('CPU budgets reject invalid controls and retain the 1 ms minimum quota',()=>{
  assert.deepEqual(budgetOptions('unlimited'),{label:'unlimited',quota:-1,period:4000});
  assert.equal(budgetOptions('0.25').quota,1000);assert.equal(budgetOptions('1.5').quota,6000);
  for(const bad of ['0','0.249','2.1','NaN','Infinity','1e0','-1','0.5;echo','0.25001','--privileged'])assert.throws(()=>budgetOptions(bad));
});

test('budget CLI protects active Unicode worktrees when a deleted worktree record remains',async()=>{
  const folder=await mkdtemp(path.join(os.tmpdir(),'pc98-budget-paths-'));
  const execute=promisify(execFile),primary=path.join(folder,'primary');
  const foreign=path.join(folder,'別の作業ツリー'),deleted=path.join(folder,'deleted');
  const config=path.join(folder,'gitconfig');
  try{
    await mkdir(primary);await writeFile(config,'');
    const env={...process.env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:config};
    const git=args=>execute('git',args,{cwd:primary,env});
    await git(['init','-q']);
    await git(['-c','user.name=Path Test','-c','user.email=path-test@example.invalid','-c','commit.gpgsign=false','commit','--allow-empty','-qm','Path fixture']);
    for(const target of [foreign,deleted])await git(['worktree','add','--detach',target]);
    await rm(deleted,{recursive:true});
    const outside=path.join(folder,'outside.hdi'),inside=path.join(foreign,'inside.hdi');
    await writeFile(outside,'');await writeFile(inside,'');
    const tool=fileURLToPath(new URL('../tools/measure_cpu_budget.mjs',import.meta.url));
    for(const [media,output,message] of [
      [inside,path.join(folder,'result.json'),'--media must stay outside every checkout'],
      [outside,path.join(foreign,'result.json'),'--output must stay outside every checkout'],
    ])await assert.rejects(execute(process.execPath,[tool,'--media='+media,'--output='+output],{cwd:primary,env,timeout:10000}),
      error=>error.code===1&&error.stderr.includes(message));
  }finally{await rm(folder,{recursive:true,force:true});}
});

test('calibration Wasm matches an independent recurrence and modular exponentiation',async()=>{
  const {run}=new WebAssembly.Instance(await WebAssembly.compile(calibrationModule())).exports;
  let value=1;
  for(let count=0;count<1000;count++){
    assert.equal(run(count)>>>0,value);assert.equal(expectedCalibration(count),value);
    value=(Math.imul(3,value)+1)>>>0;
  }
  for(const count of [1000000,30000000])assert.equal(run(count)>>>0,expectedCalibration(count));
});

test('playback summaries preserve time/sample units and reject resets or invalid windows',()=>{
  const first={wallMs:1000,guestSeconds:2,frames:10,underflows:0,overflows:0,droppedSamples:0,sequenceErrors:0,checksumErrors:0,
    guestSlices:200,guestSliceMs:400,hddReads:15,fmKeyEdges:20,audioState:'running',audioEpoch:1,running:true,error:'',
    scheduledSamples:100,deviceSampleRate:44100,completedInstructions:1000,jit:{instructions:500,compiled:1,compilationMs:10}};
  const last={...first,wallMs:4000,guestSeconds:5,frames:190,guestSlices:500,guestSliceMs:1600,hddReads:25,fmKeyEdges:30,
    scheduledSamples:132000,underflows:400,completedInstructions:3000,jit:{instructions:1500,compiled:2,compilationMs:15}};
  const result=playbackSummary([first,last]);
  assert.equal(result.guestRatio,1);assert.equal(result.fps,60);assert.equal(result.sliceMeanMs,4);assert.equal(result.reads,10);
  assert.equal(result.deviceClockRatio,1);assert.equal(result.missingSamples,400);assert.equal(result.translatedFraction,0.5);
  assert.equal(result.compilations,1);assert.equal(result.compilationWallMs,5);
  for(const change of [{audioEpoch:2},{running:false},{hidden:true},{error:'fault'},{rendererFailure:'failed'},{frames:1},{guestSeconds:NaN}])assert.throws(()=>playbackSummary([first,{...last,...change}]));
  assert.throws(()=>playbackSummary([first]));assert.throws(()=>playbackSummary([first,{...last,wallMs:1000}]));
  assert.equal(percentile([3,1,2],0.95),3);
});
