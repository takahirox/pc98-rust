import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
const source = readFileSync(new URL('../web/audio-worklet.js',import.meta.url),'utf8');
function sink(rate=48000, prebuffer=1) {
  let Processor;
  vm.runInNewContext(source, {sampleRate:rate, Float32Array,
    AudioWorkletProcessor:class { constructor() { this.port = {}; } },
    registerProcessor:(_name, cls) => { Processor = cls; },
  });
  const processor = new Processor(), reports = [];
  processor.prebufferSamples = prebuffer;
  processor.port.onmessage({data:{type:'connect',port:{postMessage:m => reports.push(m),start(){}}}});
  const send = (values,epoch=0,start=processor.receivedSamples) => processor.receive({type:'pcm',epoch,start,buffer:Float32Array.from(values).buffer,count:values.length});
  const read = count => {const out = new Float32Array(count); processor.process([],[[out]]); return [...out];};
  return {processor,reports,send,read};
}
test('PCM stays ordered across blocks, silence underflows, flush rejects old epochs and preserves first new sample', () => {
  const {processor,reports,send,read} = sink();
  send([1,2,3]); send([4,5]);
  assert.deepEqual(read(7),[1,2,3,4,5,0,0]);
  send([6,7]); assert.deepEqual(read(1),[6]);
  processor.receive({type:'flush',epoch:1});
  send([8,9],0); send([10,11],1);
  assert.deepEqual(read(3),[10,11,0]);
  assert.equal(reports.filter(m=>m.buffer).length,5);
  assert.equal(reports.find(m=>m.type==='flushed').epoch,1);
});
test('fixed six-block capacity drops newest overflow without reordering retained PCM', () => {
  const {processor,send,read} = sink();
  for(let n=1;n<=20;n++) send([n]);
  assert.equal(processor.queue.length,6);
  assert.equal(processor.overflows,14);
  assert.deepEqual(read(8),[1,2,3,4,5,6,0,0]);
});
test('device sample rates preserve source cadence and phase across render quanta', () => {
  const {send,read} = sink(96000);
  send([1,2,3]); send([4]);
  assert.deepEqual(read(3),[1,1,2]);
  assert.deepEqual(read(6),[2,3,3,4,4,0]);
  const lower=sink(24000); lower.send([1,2,3]); lower.send([4,5,6]);
  assert.deepEqual(lower.read(4),[1,3,5,0]);
});
for (const rate of [44100,48000,96000]) test(`${rate} Hz: sustained PCM matches an independent rational source cursor`,()=>{
  const {processor,send,read}=sink(rate,2400);
  let sent=0, rendered=0;
  for(let quantum=0;quantum<Math.ceil(rate*3/128);quantum++) {
    while(processor.queuedSamples<2880 && processor.queue.length<6) {
      send(Array.from({length:480},(_,i)=>sent+i+1)); sent+=480;
    }
    const output=read(128);
    for(let i=0;i<128;i++) assert.equal(output[i],Math.floor((rendered+i)*48000/rate)+1);
    rendered+=128;
    assert.equal(processor.consumedSamples,Math.floor(rendered*48000/rate));
    assert.equal(processor.receivedSamples-processor.consumedSamples,processor.queuedSamples);
    assert(processor.queuedSamples<3360);
  }
  assert.equal(processor.underflows,0); assert.equal(processor.overflows,0);
  assert.equal(processor.sequenceErrors,0);
});
test('bounded prefill and underrun recovery preserve every queued sample and report prolonged starvation',()=>{
  const {processor,reports,send,read}=sink(48000,2400);
  send(Array(480).fill(1)); assert(read(128).every(n=>n===0));
  send(Array(480).fill(2)); assert(read(128).every(n=>n===0));
  send(Array(480).fill(3)); send(Array(480).fill(4));
  assert(read(128).every(n=>n===0));
  send(Array(480).fill(5));
  assert.deepEqual(read(2400),Array.from({length:2400},(_,i)=>Math.floor(i/480)+1));
  assert.equal(processor.underflows,0); // Startup silence is separate from starvation.
  assert(read(13000).every(n=>n===0));
  assert.equal(reports.at(-1).stats.underflows,13000); // Reports even with no credits to return.
  assert.equal(reports.at(-1).stats.underflowRun,13000);
  assert.equal(processor.maxUnderflowRun,13000);
  send(Array(480).fill(6)); assert(read(128).every(n=>n===0));
  send(Array(480).fill(7)); send(Array(480).fill(8)); send(Array(480).fill(9)); send(Array(480).fill(10));
  assert.deepEqual(read(2400),Array.from({length:2400},(_,i)=>Math.floor(i/480)+6));
  assert.equal(processor.consumedSamples,4800);
  assert.equal(reports.at(-1).stats.underflowRun,0);
  assert(reports.at(-1).stats.underflows>13000); // History survives recovery.
  processor.receive({type:'flush',epoch:2});
  assert.equal(processor.consumedSamples,0); assert.equal(processor.queuedSamples,0);
  assert.equal(reports.at(-1).stats.underflowRun,0);
  const missed=processor.underflows;
  read(128); assert.equal(processor.underflows,missed); // Paused/muted sink is inactive.
  send([7],1); assert.equal(processor.staleBlocks,1); assert.equal(processor.overflows,0);
});
test('short starvation and recovery report immediately between ordinary credit messages',()=>{
  const {processor,reports,send,read}=sink(48000,2400);
  for(let n=0;n<5;n++) send(Array(480).fill(1));
  read(2400);
  const before=reports.length;
  assert(read(128).every(value=>value===0)); // Much shorter than the 250 ms periodic report.
  assert.equal(reports.length,before+1);
  assert.equal(reports.at(-1).type,'stats');
  assert.equal(reports.at(-1).stats.underflowRun,1);
  for(let n=0;n<5;n++) send(Array(480).fill(2));
  const starved=reports.length;
  assert(read(128).every(value=>value===2)); // No block is yet complete to return a credit.
  assert.equal(reports.length,starved+1);
  assert.equal(reports.at(-1).type,'stats');
  assert.equal(reports.at(-1).stats.underflowRun,0);
  assert.equal(reports.at(-1).stats.underflows,128);
  assert.equal(processor.consumedSamples,2528);
});

test('source numbering and independently computed PCM checksums expose missing or corrupted blocks',()=>{
  const {processor,read}=sink();
  const buffer=Float32Array.from([1,2,3]).buffer;
  let checksum=2166136261;
  for(const bits of new Uint32Array(buffer)) checksum=Math.imul(checksum^bits,16777619)>>>0;
  new Float32Array(buffer)[1]=4;
  processor.receive({type:'pcm',epoch:0,start:10,count:3,buffer,checksum});
  read(3);
  assert.equal(processor.sequenceErrors,1); assert.equal(processor.checksumErrors,1);
  assert.equal(processor.checkedBlocks,1);
});
