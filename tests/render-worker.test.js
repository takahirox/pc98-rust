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
  r.run('onmessage({data:{type:"frameAck",buffer:ack,sequence:1}})');
  assert.equal(r.credits.length,1);
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
  let cycles=0;
  const context=vm.createContext({TextDecoder,ArrayBuffer,Uint8Array,Float32Array,Uint32Array,
    performance:{now:()=>10,timeOrigin:100},setTimeout:()=>1,clearTimeout(){},
    postMessage:m=>events.push(m),
    MessageChannel:class {constructor(){this.port1={};this.port2={postMessage(){}};}},
    testCore:{memory:{buffer:new ArrayBuffer(640*400*4)},metric:id=>id===5?cycles:0,peek:()=>0,
      video_snapshot:()=>0,video_snapshot_len:()=>219952,frame:()=>0},
    testPort:{start(){},close(){},postMessage:(m,transfers)=>frames.push(structuredClone(m,{transfer:transfers}))},
  });
  const run=code=>vm.runInContext(code,context);
  run(emulatorSource.replace('import.meta.url',"'file:///emulator-worker.js'"));
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
  run("audioEnabled=true;audioState='running';port={};sentSamples=0;frameDue=0;presentDue()");
  assert.equal(frames.length,2);
  run('sentSamples=2880;presentDue()');
  assert.equal(frames.length,3);
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
