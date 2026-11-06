import {test} from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import {readFileSync} from 'node:fs';
const source = readFileSync(new URL('../web/audio-worklet.js',import.meta.url),'utf8');
function sink(rate=48000) {
  let Processor;
  vm.runInNewContext(source, {sampleRate:rate, Float32Array,
    AudioWorkletProcessor:class { constructor() { this.port = {}; } },
    registerProcessor:(_name, cls) => { Processor = cls; },
  });
  const processor = new Processor(), reports = [];
  processor.port.onmessage({data:{type:'connect',port:{postMessage:m => reports.push(m),start(){}}}});
  const send = (values,epoch=0) => processor.receive({type:'pcm',epoch,buffer:Float32Array.from(values).buffer,count:values.length});
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
