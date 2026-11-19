import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';

const source = readFileSync(new URL('../web/render-worker.js',import.meta.url),'utf8');
function renderer() {
  const frames=[], credits=[];
  const memory = new ArrayBuffer(2*1024*1024);
  const context = vm.createContext({ArrayBuffer,Uint8Array,DataView,performance:{now:()=>10,timeOrigin:100},
    postMessage:(m,transfers=[])=>frames.push(structuredClone(m,{transfer:transfers})),
    testCore:{memory:{buffer:memory},input_buffer:()=>640*400*4,
      video_render:len=>len===219952 ? 8 : 0},
    testPort:{start(){},postMessage:(m,transfers)=>credits.push(structuredClone(m,{transfer:transfers}))},
  });
  vm.runInContext(source.replace('import.meta.url', "'file:///render-worker.js'"),context);
  vm.runInContext('core=testCore',context);
  const run=code=>vm.runInContext(code,context);
  run('onmessage({data:{type:"connect",port:testPort}})');
  const snapshot=sequence=> {
    context.packet={buffer:new ArrayBuffer(547632),length:219952,sequence,captured:100,videoEpoch:3,generation:1,guestCycles:0};
    run('testPort.onmessage({data:packet})');
  };
  return {run,context,snapshot,frames,credits};
}
test('one snapshot and one pixel credit bound traffic until presentation; returned buffers are recycled',()=>{
  const r=renderer();
  r.snapshot(1);
  assert.equal(r.frames.length,1); assert.equal(r.frames[0].event,'frame');
  assert.equal(r.context.packet.buffer.byteLength,547632); // Renderer still owns source snapshot.
  assert.equal(r.run('pixels'),null);
  assert.equal(r.credits.length,0); // No next snapshot before DOM acknowledges.
  r.context.ack=r.frames[0].buffer;
  r.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:1,captureAt:120}})');
  assert.equal(r.credits.length,1);
  assert.equal(r.credits[0].captureAt,120);
  assert.equal(r.credits[0].buffer.byteLength,547632);
  assert.equal(r.context.packet.buffer.byteLength,0); // Real transferable detachment.
  r.snapshot(2);
  assert.equal(r.frames[1].event,'frame');
  assert.equal(r.frames[1].renderStats.frames,2);
});
test('overlapping snapshots, reversed sequences, invalid snapshots and duplicate credits fail closed',()=>{
  const overlapping=renderer(); overlapping.snapshot(2); overlapping.snapshot(3);
  assert.equal(overlapping.frames.at(-1).event,'fault');
  const reversed=renderer(); reversed.snapshot(2);
  reversed.context.ack=reversed.frames[0].buffer;
  reversed.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:2}})');
  reversed.snapshot(1);
  assert.equal(reversed.frames.at(-1).event,'fault');
  const invalid=renderer();
  invalid.run('testPort.onmessage({data:{buffer:new ArrayBuffer(4),length:5,sequence:1}})');
  assert.equal(invalid.frames.at(-1).event,'fault');
  const mismatched=renderer();
  mismatched.context.packet={buffer:new ArrayBuffer(547632),length:219952,sequence:1,videoEpoch:1,generation:1,guestCycles:7};
  mismatched.run('testPort.onmessage({data:packet})');
  assert.equal(mismatched.frames.at(-1).event,'fault');
  const credit=renderer(); credit.snapshot(1);
  credit.run('onmessage({data:{type:"frameAck",buffer:new ArrayBuffer(640*400*4),sequence:9}})');
  assert.equal(credit.frames.at(-1).event,'fault');
  assert.equal(credit.credits.length,0);
});

const emulatorSource=readFileSync(new URL('../web/emulator-worker.js',import.meta.url),'utf8');
test('authoritative Worker coalesces full snapshots, rejects stale credits and replaces only fault-owned buffers',async()=>{
  const frames=[],events=[];
  let cycles=0, now=10, capture;
  const context=vm.createContext({TextDecoder,ArrayBuffer,Uint8Array,Float32Array,Uint32Array,
    performance:{now:()=>now,timeOrigin:100},setTimeout:fn=>{capture=fn;return 1;},clearTimeout(){},
    postMessage:m=>events.push(m),
    MessageChannel:class {constructor(){this.port1={};this.port2={postMessage(){}};}},
    testCore:{memory:{buffer:new ArrayBuffer(640*400*4)},metric:id=>id===5?cycles:0,peek:()=>0,
      video_snapshot:()=>0,video_snapshot_len:()=>219952,frame:()=>0},
    testPort:{start(){},close(){},postMessage:(m,transfers)=>frames.push(structuredClone(m,{transfer:transfers}))},
  });
  const run=code=>vm.runInContext(code,context);
  run(emulatorSource.replaceAll('import.meta.url',"'file:///emulator-worker.js'"));
  run('core=testCore');
  await run("command('videoPort',testPort)");
  assert.equal(frames.length,1);
  for(let i=0;i<100;i++) {cycles++;run('present()');}
  assert.equal(frames.length,1); // No serialized snapshots queue behind a slow renderer.
  assert.equal(run('snapshotBuffer'),null);
  context.credit=frames[0].buffer;
  run('testPort.onmessage({data:{sequence:-1,buffer:credit}})');
  assert.equal(run('snapshotBuffer'),null);
  run('invalidateVideo()'); // A control barrier occurs while the first snapshot is away.
  run('testPort.onmessage({data:{sequence:1,buffer:credit}})');
  assert.equal(frames.length,2);
  assert.equal(frames[1].guestCycles,100); // Rebuild from newest authoritative state.
  assert(frames[1].videoEpoch>frames[0].videoEpoch);
  assert.equal(context.credit.byteLength,0);
  assert.equal(run('frameBuffer'),null); // Default path retains no unused pixel transfer.
  context.credit=frames[1].buffer;
  run('testPort.onmessage({data:{sequence:2,buffer:credit}})');
  // Snapshot capture is cheap, but the render Worker still shares the host
  // CPU budget. A depleted PCM reserve defers launching either render path.
  cycles++;
  run("audioEnabled=true;audioState='running';port={};sentSamples=0;presentDue()");
  assert.equal(frames.length,2);
  run('sentSamples=2880;presentDue()');
  assert.equal(frames.length,2); // The sink consumed PCM while this cursor aged.
  run('sinkStatsAt=performance.now();presentDue()');
  assert.equal(frames.length,3);

  // The display credit wakes capture without advancing the guest. An early
  // credit waits until its deadline and a repeated guest clock stays idle.
  context.credit=frames[2].buffer;
  cycles++;
  run('running=true;sentSamples=3120;testPort.onmessage({data:{sequence:3,buffer:credit,captureAt:115}})');
  assert.equal(frames.length,3);
  now=14; run('presentDue()');
  assert.equal(frames.length,3);
  now=15; capture();
  assert.equal(frames.length,4);
  assert.equal(frames[3].guestCycles,cycles);
  context.credit=frames[3].buffer;
  run('testPort.onmessage({data:{sequence:4,buffer:credit}})');
  assert.equal(frames.length,4);

  run('testPort.onmessageerror()');
  assert.equal(events.at(-1).event,'renderFault');
  await run("command('videoFallback')");
  assert.equal(run('videoPort'),null);
  assert.equal(run('snapshotBuffer'),null);
  assert.equal(events.at(-1).event,'frame');
  assert.equal(events.at(-1).buffer.byteLength,640*400*4);
  const count=events.length;
  await run("command('videoFallback')");
  assert.equal(events.length,count);
  assert.equal(run('frameBuffer'),null);
});

test('unchanged raster proofs name the preceding output and retain full pixel credits',()=>{
 const r=renderer();
 r.run('testCore.video_render_changed=()=>1;new Uint8Array(testCore.memory.buffer)[8]=31');
 r.snapshot(1);
 assert.equal(r.frames[0].samePixelsAs,0);
 r.context.ack=r.frames[0].buffer;r.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:1}})');
 r.run('testCore.video_render_changed=()=>0');r.snapshot(2);
 assert.equal(r.frames[1].samePixelsAs,1);assert.equal(r.frames[1].buffer.byteLength,640*400*4);
 assert.equal(new Uint8Array(r.frames[1].buffer)[0],31);
 assert.equal(r.frames[1].renderStats.unchangedFrames,1);
 r.context.ack=r.frames[1].buffer;r.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:2}})');
 r.run('testCore.video_render_changed=()=>1;new Uint8Array(testCore.memory.buffer)[8]=83');r.snapshot(3);
 assert.equal(r.frames[2].samePixelsAs,0);
 r.context.ack=r.frames[2].buffer;r.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:3}})');
 r.run(`const grown=new ArrayBuffer(3*1024*1024);
   new Uint8Array(grown).set(new Uint8Array(testCore.memory.buffer));
   testCore.memory.buffer=grown;testCore.video_render_changed=()=>0`);
 r.snapshot(4);
 assert.equal(r.frames[3].samePixelsAs,3);assert.equal(r.frames[3].buffer.byteLength,640*400*4);
 assert.equal(new Uint8Array(r.frames[3].buffer)[0],83);
});

// Exercise the presentation consumer independently of renderer/guest mocks.
// A proof for a discarded output must never suppress an actual canvas update.
import {Presenter} from '../web/presenter.js';
test('presentation only reuses its accepted lineage and redraws after barriers or raw context loss',()=>{
 const saved=new Map(['ResizeObserver','window','matchMedia','devicePixelRatio'].map(k=>[k,Object.getOwnPropertyDescriptor(globalThis,k)]));
 try{
  globalThis.ResizeObserver=class {observe(){}};globalThis.window={addEventListener(){}};globalThis.devicePixelRatio=1;
  globalThis.matchMedia=()=>({addEventListener(){}});
  const puts=[],draws=[],raw={width:2,height:1,getContext:()=>({putImageData:f=>puts.push([...f.data])})};
  const p=new Presenter(raw,{addEventListener(){}},{},()=>{});p.draw=bytes=>draws.push(bytes);
  const worker={},other={},image=n=>({width:2,height:1,data:Uint8ClampedArray.of(n,0,0,255,n,0,0,255)});
  const source=(sequence,samePixelsAs,extra={})=>({renderer:worker,sequence,samePixelsAs,generation:1,videoEpoch:1,...extra});
  p.present(image(1),source(1,0));p.present(image(1),source(2,1));
  assert.equal(puts.length,1);assert.equal(draws.length,1);assert.equal(p.stats.reusedFrames,1);
  // Output 3 changed, but was discarded before presentation.
  p.present(image(2),source(4,3));assert.equal(puts.length,2);assert.equal(puts.at(-1)[0],2);
  p.present(image(2),source(5,4,{videoEpoch:2}));assert.equal(puts.length,3);
  p.present(image(2),source(6,5,{videoEpoch:2}));assert.equal(puts.length,3);
  p.present(image(2),source(7,6,{videoEpoch:2,generation:2}));assert.equal(puts.length,4);
  p.present(image(2),source(8,7,{renderer:other,videoEpoch:2,generation:2}));assert.equal(puts.length,5);
  p.present(image(2));p.present(image(2),source(10,9));assert.equal(puts.length,7);
  p.invalidate();p.redraw();assert.equal(draws.length,7);
  p.present(image(2),source(11,10));assert.equal(puts.length,8);
  raw.width=1;p.present(image(2),source(12,11));assert.equal(puts.length,9);
  p.setPreset('off');assert.equal(draws.length,10); // UI redraws are never suppressed.
 }finally{for(const[k,d]of saved)d?Object.defineProperty(globalThis,k,d):delete globalThis[k];}
});
