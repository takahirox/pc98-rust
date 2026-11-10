import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';

const source = readFileSync(new URL('../web/emulator-worker.js',import.meta.url),'utf8');

// Exercise the production scheduler with a 7 ms cost per 10 ms guest slice.
// This host can produce PCM in real time, but cannot also pay a nested timer's
// 4 ms minimum on every slice. No wall-clock sleeps or platform speed thresholds.
function worker(code=source, {prebuffer=2400, stalls=[], frameCost=0, renderWorker=false, sliceCost=()=>7}={}) {
  let now=0, nextId=0, samples=0, started=false, missed=0, slices=0, clocks=0;
  const frames=[];
  const events=[], blocks=[], recycled=[];
  const sink={audioEpoch:0,consumedSamples:0};
  let consumed=0, reported=0, underflowRun=0;
  const post = (fn, delay=0) => {
    const event={id:++nextId,at:now+delay,fn}; events.push(event); return event.id;
  };
  function advance(time) {
    let remaining=(time-now)*48;
    if (started) {
      const requested=remaining;
      while(remaining && blocks.length) {
        const block=blocks[0], n=Math.min(remaining,block.count);
        block.count-=n; remaining-=n; consumed+=n;
        if (!block.count) {
          // Production reports the source cursor with a returned block, not
          // through a shared counter that exposes partially consumed blocks.
          reported=consumed;
          recycled.push(blocks.shift().buffer);
        }
      }
      missed+=remaining;
      if (remaining) underflowRun = remaining < requested ? remaining : underflowRun+remaining;
      else if (requested) underflowRun=0;
    }
    now=time;
  }
  const context=vm.createContext({
    TextDecoder, ArrayBuffer, Float32Array, Uint8Array, Uint32Array,
    performance:{now:()=>now}, postMessage:m=>{
      if(m.event==='frame') post(()=>run('onmessage({data:{type:"frameAck",data:new ArrayBuffer(640*400*4)}})'));
    },
    setTimeout:(fn,delay)=>post(fn,Math.max(4,delay)),
    clearTimeout:id=>{const e=events.find(e=>e.id===id);if(e)e.cancelled=true;},
    MessageChannel:class {
      constructor() {
        this.port1={}; this.port2={postMessage:data=>post(()=>this.port1.onmessage({data}))};
      }
    },
    testSink:sink, recycled,
    testVideoPort:{postMessage:()=>{
      frames.push(now);
      // Another Worker is independently scheduled, but shares a finite host
      // CPU budget. Charge its contention in a later task, after PCM delivery.
      post(()=>{advance(now+frameCost);run('snapshotBuffer=new ArrayBuffer(547632)');});
    }},
    testPort:{start(){},postMessage:m=>{
      if (m.type !== 'pcm') return;
      blocks.push({count:m.count,buffer:m.buffer});
      if(blocks.reduce((sum,b)=>sum+b.count,0)>=prebuffer) started=true;
    }},
    testCore:{memory:{buffer:new ArrayBuffer(640*400*4)},audio_ptr:()=>0,peek:()=>0,
      frame:()=>{frames.push(now);advance(now+frameCost);return 0;},
      video_snapshot:()=>0,video_snapshot_len:()=>547632,
      audio_len:()=>samples,audio_clear:()=>{samples=0;},metric:id=>id===5?clocks:0,
      step_scheduler:(budget, cap)=>{assert.equal(budget,80000);assert.equal(cap,100000);slices++;advance(now+sliceCost(slices));samples=480;clocks+=80000;}},
  });
  vm.runInContext(code.replaceAll('import.meta.url', "'file:///emulator-worker.js'"),context);
  const run=code=>vm.runInContext(code,context);
  run("core=testCore;port=testPort;sinkStats=testSink;running=true;audioEnabled=true;audioState='running';frameDue="+(frameCost?'0':'Infinity')+";");
  if(renderWorker) run('videoPort=testVideoPort;snapshotBuffer=new ArrayBuffer(547632)');
  for (const [at,duration] of stalls) post(()=>advance(now+duration),at);
  function next() {
    events.sort((a,b)=>a.at-b.at || a.id-b.id);
    const event=events.shift(); if(!event) return false;
    advance(Math.max(now,event.at));
    if (recycled.length) run('sinkStatsAt=performance.now()');
    sink.consumedSamples=reported;
    sink.underflows=missed;
    sink.underflowRun=underflowRun;
    run('audioBuffers.push(...recycled.splice(0))');
    if(!event.cancelled) event.fn();
    return true;
  }
  return {run,next,until:time=>{
    for(let tasks=0;now<time;tasks++) {
      assert(tasks<10000,'Worker spins without advancing time');
      assert(next(),'Worker stopped scheduling');
    }
  },
    result:()=>({now,missed,slices,frames,produced:run('producedSamples'),outstanding:run('outstandingAudio()')})};
}

test('host rendering waits for PCM reserve while preview refresh stays bounded',()=>{
  const current=worker(source,{frameCost:22}); current.run('schedule(0)'); current.until(5000);
  assert.equal(current.result().missed,0);
  assert(current.result().produced>=48000*4.95);
  const frames=current.result().frames;
  assert(frames.length>30);
  // With a sustainable guest cost, reserve-based rendering still refreshes
  // regularly despite the independently scheduled 22 ms render job.
  const longestFrameGap=Math.max(...frames.slice(1).map((at,i)=>at-frames[i]));
  assert(longestFrameGap<=167,`frame gap ${longestFrameGap}`);
  assert(current.result().outstanding<3360);
  const old=worker(source.replace('if (reserve < (audioBuffers.length ? 2880 : 2400) && !starvedPreview) return;',''),{frameCost:22});
  old.run('schedule(0)'); old.until(5000);
  assert(old.result().missed>48000*0.3);
});

test('independent rasterization waits for PCM reserve on a CPU-constrained host',()=>{
  const options={frameCost:22,renderWorker:true};
  const current=worker(source,options); current.run('schedule(0)'); current.until(5000);
  assert.equal(current.result().missed,0);
  assert(current.result().produced>=48000*4.95);
  assert(current.result().frames.length>30);
  const frames=current.result().frames;
  assert(Math.max(...frames.slice(1).map((at,i)=>at-frames[i]))<=167);
  assert(current.result().outstanding<3360);
  // Negative control: the old path launched render work with a depleted sink
  // just because snapshot capture in the producer itself was inexpensive.
  const old=worker(source.replace('if (reserve < (audioBuffers.length ? 2880 : 2400) && !starvedPreview)',
    'if (!videoPort && reserve < (audioBuffers.length ? 2880 : 2400) && !starvedPreview)'),options);
  old.run('schedule(0)'); old.until(5000);
  assert(old.result().missed>48000*0.3);
});

test('a slow guest scene cannot force render work to spend the remaining PCM reserve',()=>{
  const options={frameCost:22,renderWorker:true,
    sliceCost:slice=>slice>=95 && slice<115 ? 11 : 7};
  const current=worker(source,options); current.run('schedule(0)'); current.until(3000);
  assert.equal(current.result().missed,0);
  assert(current.result().outstanding<3360);
  assert(current.result().frames.at(-1)>2800,'Rendering resumes after PCM catches up');
  // Same bounded scene cost, guest time, credits and reserve. The old wall
  // deadline launched a renderer while the producer was still behind.
  const old=worker(source.replace('if (reserve < (audioBuffers.length ? 2880 : 2400) && !starvedPreview) return;',
    'if (playingAudio() && outstandingAudio() < 2880 && now < frameDue+100) return;'),options);
  old.run('schedule(0)'); old.until(3000);
  assert(old.result().missed>0);
});

test('a persistently starved sink keeps a preview without claiming continuous audio',()=>{
  const options={frameCost:22,renderWorker:true,sliceCost:()=>20};
  const current=worker(source,options); current.run('schedule(0)'); current.until(3000);
  assert(current.result().missed>48000*0.3); // The host cannot meet the guest clock.
  assert(current.result().frames.length>5);
  const frozen=worker(source.replace(' && !starvedPreview',''),options);
  frozen.run('schedule(0)'); frozen.until(3000);
  assert(frozen.result().frames.length<2);
});

test('current sink starvation permits bounded previews despite a positive stale reserve',()=>{
  const old=source.replace('sinkStats.audioEpoch === audioEpoch && sinkStats.underflowRun > 0',
    'sinkStats.underflows > 0 && reserve === 0');
  for (const [code,expected] of [[source,1],[old,0]]) {
    const w=worker(code,{renderWorker:true});
    // A delayed cursor overestimates 30 ms of PCM while the sink has already
    // emitted silence. Waiting for that estimate to hit exactly zero freezes it.
    w.run('sentSamples=1440;sinkStats.underflows=1000;sinkStats.underflowRun=1000;frameDue=-99;presentDue()');
    assert.equal(w.result().frames.length,0);
    w.run('frameDue=-100;presentDue()');
    assert.equal(w.result().frames.length,expected);
    assert.equal(w.result().slices,0); // Rendering does not advance guest time.
  }
});

test('recovered and previous-epoch underruns cannot bypass a healthy stream reserve',()=>{
  for (const [epoch,run] of [[0,0],[-1,1000]]) {
    const w=worker(source,{renderWorker:true});
    w.run(`sinkStats.audioEpoch=${epoch};sinkStats.underflows=1000;sinkStats.underflowRun=${run};frameDue=-100;presentDue()`);
    assert.equal(w.result().frames.length,0);
  }
});

test('a missing-PCM edge between guest slices refreshes the preview before recovery overwrites it',async()=>{
  for (const [code,expected] of [[source,1],[source.replace('if (activeStarvation) presentDue();',''),0]]) {
    const w=worker(code,{renderWorker:true});
    const connected=w.run("command('audioPort',testPort)");
    w.run("testPort.onmessage({data:{type:'flushed',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflows:1000,underflowRun:0}}})");
    await connected;
    w.run('sentSamples=1440;frameDue=-100;presentDue()');
    assert.equal(w.result().frames.length,0);
    w.run("testPort.onmessage({data:{type:'stats',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflows:1001,underflowRun:1}}})");
    w.run("testPort.onmessage({data:{type:'stats',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflows:1128,underflowRun:0}}})");
    w.run('presentDue()');
    assert.equal(w.result().frames.length,expected);
    assert.equal(w.result().slices,0);
  }
});

test('starvation notifications do not refresh during pause, inspection or media operations',async()=>{
  for (const state of ['running=false','manual=true','busy=true',"transaction='media'"]) {
    const w=worker(source,{renderWorker:true});
    const connected=w.run("command('audioPort',testPort)");
    w.run("testPort.onmessage({data:{type:'flushed',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflowRun:0}}})");
    await connected;
    w.run(`frameDue=-100;${state}`);
    w.run("testPort.onmessage({data:{type:'stats',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflowRun:1}}})");
    assert.equal(w.result().frames.length,0);
    assert.equal(w.result().slices,0);
  }
});

test('a flushed stream cannot use a previous epoch underflow to spend its new prefill',async()=>{
  const w=worker(source,{renderWorker:true});
  const connected=w.run("command('audioPort',testPort)");
  w.run("testPort.onmessage({data:{type:'flushed',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0,underflows:1000}}})");
  await connected;
  w.run('frameDue=-1000;presentDue()');
  assert.equal(w.result().frames.length,0);
});

test('audio catch-up yields finite tasks without the nested timer producer deficit',()=>{
  const current=worker(); current.run('schedule(0)'); current.until(5000);
  assert.equal(current.result().missed,0);
  assert(current.result().produced>=48000*4.95);
  assert(current.result().outstanding<3360);
  // Negative control keeps the same credits, watermark, sink and guest cost;
  // only restore the previous timer yield. Prefilling cannot hide its deficit.
  const old=worker(source.replace('? 0 : 5)', '? 1 : 5)'));
  old.run('schedule(0)'); old.until(5000);
  assert(old.result().missed>48000*0.3);
  assert(old.result().produced<48000*4.6);
});

test('the finite reserve survives isolated 35 ms scheduling stalls without hiding a producer deficit',()=>{
  const stalls=[[1000,35],[3000,35]];
  const current=worker(source,{stalls}); current.run('schedule(0)'); current.until(5000);
  assert.equal(current.result().missed,0);
  assert(current.result().produced>=48000*4.95);
  assert(current.result().outstanding<3360);
  // Same slice cost and stalls, but the former 40 ms target/30 ms prefill.
  // This isolates a reserve shortfall from the sustained timer deficit above.
  const small=worker(source.replaceAll('2880','1920'),{prebuffer:1440,stalls});
  small.run('schedule(0)'); small.until(5000);
  assert(small.result().missed>0);
});

test('pause cancels a queued task, resume ignores stale tasks, and a full sink waits',()=>{
  const w=worker();
  w.run('schedule(0);running=false;schedule();'); w.next();
  assert.equal(w.result().slices,0);
  w.run('running=true;schedule(0);running=false;schedule();running=true;schedule(0);');
  w.next(); assert.equal(w.result().slices,0); // Stale task cannot run the resumed schedule.
  w.next(); assert.equal(w.result().slices,1);
  const full=worker(); full.run('sentSamples=2880;schedule(0)'); full.next();
  assert.equal(full.result().slices,0);
  full.next();
  assert(full.result().now>=5); // No immediate spin once the watermark is full.
  assert.equal(full.result().slices,0);
});

test('a current sink credit preempts polling, while stale and full cursors do not',async()=>{
  const w=worker();
  w.run('running=false');
  const connected=w.run("command('audioPort',testPort)");
  w.run("testPort.onmessage({data:{type:'flushed',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0}}})");
  await connected;
  w.run('running=true;sentSamples=2880;schedule(5)');
  w.run("testPort.onmessage({data:{type:'recycle',epoch:audioEpoch-1,stats:{audioEpoch:audioEpoch-1,consumedSamples:480}}})");
  w.run("testPort.onmessage({data:{type:'stats',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:0}}})");
  assert.equal(w.run("timer === 'task'"),false);
  w.run("testPort.onmessage({data:{type:'recycle',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:480}}})");
  w.next();
  assert.equal(w.result().slices,1);
  assert.equal(w.result().now,7); // Execute immediately, without the 5 ms polling wait.
  w.next();
  assert.equal(w.result().slices,1); // The preempted timer cannot execute another slice.
  w.run('running=false;schedule()');
  w.run("testPort.onmessage({data:{type:'recycle',epoch:audioEpoch,stats:{audioEpoch,consumedSamples:960}}})");
  assert.equal(w.run('timer'),null); // Credits cannot restart paused execution.
});
