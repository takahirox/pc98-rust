import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';

const source = readFileSync(new URL('../web/emulator-worker.js',import.meta.url),'utf8');

// Exercise the production scheduler with a 7 ms cost per 10 ms guest slice.
// This host can produce PCM in real time, but cannot also pay a nested timer's
// 4 ms minimum on every slice. No wall-clock sleeps or platform speed thresholds.
function worker(code=source) {
  let now=0, nextId=0, samples=0, started=false, missed=0, slices=0;
  const events=[], blocks=[], recycled=[];
  const sink={audioEpoch:0,consumedSamples:0};
  const post = (fn, delay=0) => {
    const event={id:++nextId,at:now+delay,fn}; events.push(event); return event.id;
  };
  function advance(time) {
    let remaining=(time-now)*48;
    if (started) {
      while(remaining && blocks.length) {
        const block=blocks[0], n=Math.min(remaining,block.count);
        block.count-=n; remaining-=n; sink.consumedSamples+=n;
        if (!block.count) recycled.push(blocks.shift().buffer);
      }
      missed+=remaining;
    }
    now=time;
  }
  const context=vm.createContext({
    TextDecoder, ArrayBuffer, Float32Array, Uint8Array, Uint32Array,
    performance:{now:()=>now}, postMessage:()=>{},
    setTimeout:(fn,delay)=>post(fn,Math.max(4,delay)),
    clearTimeout:id=>{const e=events.find(e=>e.id===id);if(e)e.cancelled=true;},
    MessageChannel:class {
      constructor() {
        this.port1={}; this.port2={postMessage:data=>post(()=>this.port1.onmessage({data}))};
      }
    },
    testSink:sink, recycled,
    testPort:{postMessage:m=>{
      blocks.push({count:m.count,buffer:m.buffer});
      if(blocks.reduce((sum,b)=>sum+b.count,0)>=1440) started=true;
    }},
    testCore:{memory:{buffer:new ArrayBuffer(4096)},audio_ptr:()=>0,
      audio_len:()=>samples,audio_clear:()=>{samples=0;},metric:()=>0,
      step:budget=>{assert.equal(budget,10000);slices++;advance(now+7);samples=480;}},
  });
  vm.runInContext(code.replace('import.meta.url', "'file:///emulator-worker.js'"),context);
  const run=code=>vm.runInContext(code,context);
  run("core=testCore;port=testPort;sinkStats=testSink;running=true;audioEnabled=true;audioState='running';frameDue=Infinity;");
  function next() {
    events.sort((a,b)=>a.at-b.at || a.id-b.id);
    const event=events.shift(); if(!event) return false;
    advance(Math.max(now,event.at));
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
    result:()=>({now,missed,slices,produced:run('producedSamples'),outstanding:run('outstandingAudio()')})};
}

test('audio catch-up yields finite tasks without the nested timer producer deficit',()=>{
  const current=worker(); current.run('schedule(0)'); current.until(5000);
  assert.equal(current.result().missed,0);
  assert(current.result().produced>=48000*4.95);
  assert(current.result().outstanding<2400);
  // Negative control keeps the same credits, watermark, sink and guest cost;
  // only restore the previous timer yield. Prefilling cannot hide its deficit.
  const old=worker(source.replace('? 0 : 5)', '? 1 : 5)'));
  old.run('schedule(0)'); old.until(5000);
  assert(old.result().missed>48000*0.3);
  assert(old.result().produced<48000*4.6);
});

test('pause cancels a queued task, resume ignores stale tasks, and a full sink waits',()=>{
  const w=worker();
  w.run('schedule(0);running=false;schedule();'); w.next();
  assert.equal(w.result().slices,0);
  w.run('running=true;schedule(0);running=false;schedule();running=true;schedule(0);');
  w.next(); assert.equal(w.result().slices,0); // Stale task cannot run the resumed schedule.
  w.next(); assert.equal(w.result().slices,1);
  const full=worker(); full.run('sentSamples=1920;schedule(0)'); full.next();
  assert.equal(full.result().slices,0);
  full.next();
  assert(full.result().now>=5); // No immediate spin once the watermark is full.
  assert.equal(full.result().slices,0);
});
