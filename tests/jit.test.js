import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {compileBlock,WasmJit} from '../web/jit.js';

const module=await WebAssembly.compile(await readFile(new URL('../web/core.wasm',import.meta.url)));
const metrics=[0,1,2,3,5,6,7,9,10,14,15,19,20,23,26,27,28,30];
const packet=core=>new Uint32Array(core.memory.buffer,core.jit_packet()>>>0,core.jit_packet_len()).slice();
const layout=core=>Array.from(new Uint32Array(core.memory.buffer,core.jit_layout()>>>0,3));
function state(core){
  const [r,f,ip]=layout(core),view=new DataView(core.memory.buffer);
  return {r:Array.from(new Uint16Array(core.memory.buffer,r,8)),flags:view.getUint16(f,true),ip:view.getUint32(ip,true),
    metrics:metrics.map(i=>core.metric(i)),error:core.metric(4)};
}
function setState(core,regs,flags,ip){
  const [r,f,p]=layout(core),view=new DataView(core.memory.buffer);
  new Uint16Array(core.memory.buffer,r,8).set(regs);view.setUint16(f,flags,true);view.setUint32(p,ip,true);
}
async function boot(code,patch=[]){
  const core=new WebAssembly.Instance(module,{}).exports;
  core.set_profile(1);core.set_boot_source(0);
  const disk=new Uint8Array(1261568);disk.set([0xfa,...code]);disk.set(patch,0x100);
  const ptr=core.input_buffer(disk.length)>>>0;new Uint8Array(core.memory.buffer,ptr,disk.length).set(disk);
  assert.equal(core.insert(0,0),1);core.reset();
  for(let n=0;core.metric(17)!==0x1fc0||core.metric(18)!==1;n++){
    assert(n<2000,'original boot probe did not reach CLI');assert.equal(core.metric(4),0);core.step(1);
  }
  return core;
}
async function pair(instruction,patch=[]){
  // NOP makes a conditional branch a two-instruction block. Both outcomes
  // rejoin this original boot probe's loop; there are no external guest bytes.
  const code=[0x90,...instruction,0x90,0xeb,(-instruction.length-4)&255];
  const a=await boot(code,patch),b=await boot(code,patch);
  let id=0;
  for(let n=0;!id;n++){
    assert(n<64,'hot probe did not warm');
    for(const core of [a,b]){const [,,p]=layout(core);new DataView(core.memory.buffer).setUint32(p,1,true);}
    a.jit_begin_slice(80000,1);id=a.jit_dispatch(1);b.step(a.jit_done());
  }
  assert(id,'hot register block was not offered');
  const plan=packet(a);assert.equal(plan[2],1);
  const bytes=compileBlock(plan),compiled=await WebAssembly.compile(bytes);
  const run=new WebAssembly.Instance(compiled,{core:{memory:a.memory,next:a.jit_next}}).exports.run;
  assert.equal(a.jit_register(id,1),1);assert.deepEqual(state(a),state(b));
  return {a,b,id,plan,run};
}
function execute(p,regs,flags,cap=2){
  for(const core of [p.a,p.b])setState(core,regs,flags,1);
  p.a.jit_begin_slice(80000,cap);assert.equal(p.a.jit_start(p.id),1);
  p.run(...layout(p.a));p.b.step(cap);
  assert.equal(p.a.jit_done(),cap);
  assert.deepEqual(state(p.a),state(p.b));
}
let seed=0x98c0ffee;
function random(){seed^=seed<<13;seed^=seed>>>17;seed^=seed<<5;return seed>>>0;}

test('generated Wasm matches interpreter register ALU, carries and flags',async()=>{
  for(const wide of [0,1])for(let kind=0;kind<8;kind++)for(const reverse of [0,2]){
    const p=await pair([kind*8+wide+reverse,0xd8]); // AX/AL and BX/BL, both directions
    const values=wide?[0,1,15,16,127,128,255,256,32767,32768,65534,65535]:[0,1,15,16,127,128,254,255];
    for(const a of values)for(const b of values)for(const carry of [0,1]){
      const regs=Array.from({length:8},()=>random()&65535);regs[0]=a;regs[3]=b;
      execute(p,regs,0xcd6|carry);
    }
    for(let n=0;n<150;n++)execute(p,Array.from({length:8},()=>random()&65535),(random()&0xcd5)|2);
  }
});

test('generated Wasm preserves byte aliases, immediate sign extension and INC carry',async()=>{
  const cases=[
    [0xb4,0x81],[0xbc,0x34,0x12],[0x88,0xe0],[0x8a,0xe0],[0x89,0xe0],[0x8b,0xe0],
    [0x86,0xe0],[0x87,0xe0],[0x94],[0x40],[0x4f],[0x9e],[0x9f],[0xf5],[0xf8],[0xf9],[0xfc],[0xfd],
    [0xc6,0xc4,0x87],[0xc7,0xc7,0x34,0x12],[0x84,0xe0],[0x85,0xe0],[0xa8,0xf1],[0xa9,0x01,0x80],
  ];
  for(let kind=0;kind<8;kind++)cases.push([0x80,0xc4|kind<<3,0x80],[0x81,0xc0|kind<<3,0x01,0x80],[0x83,0xc0|kind<<3,0xff],
    [kind*8+4,0x80],[kind*8+5,0xff,0x7f]);
  for(const code of cases){
    const p=await pair(code);
    for(let n=0;n<100;n++)execute(p,Array.from({length:8},()=>random()&65535),(random()&0xcd5)|2);
  }
});

test('every condition and LOOP path retains IP, registers and taken-branch timing',async()=>{
  for(const op of [...Array.from({length:16},(_,i)=>0x70+i),0xe0,0xe1,0xe2,0xe3,0xe9,0xeb]){
    const p=await pair(op===0xe9?[op,1,0]:[op,1]);
    for(let bits=0;bits<32;bits++)for(const cx of [0,1,2,65535]){
      const flags=2|(bits&1)|((bits&2)<<5)|((bits&4)<<5)|((bits&8)<<8)|((bits&16)>>2);
      execute(p,[0,cx,0,0,0,0,0,0],flags);
    }
  }
});

test('deadline/work caps stop at each instruction, including a zero-clock device tick',async()=>{
  const p=await pair([0x40]);
  execute(p,[65535,0,0,0,0,0,0,0],3,1);
  const before=state(p.a);p.a.jit_begin_slice(0,100);assert.equal(p.a.jit_start(p.id),0);assert.equal(p.a.jit_done(),0);
  assert.deepEqual(state(p.a),before);
  p.a.jit_begin_slice(80000,0);assert.equal(p.a.jit_dispatch(1),0);assert.deepEqual(state(p.a),before);
});

test('compilation failure keeps executing the interpreter; reset rejects an old offer',async()=>{
  const core=await boot([0x90,0x40,0xeb,0xfc]);
  const jit=new WasmJit(core,{compile:()=>Promise.reject(Error('blocked compilation'))});
  jit.step(80000,10000);await new Promise(resolve=>setTimeout(resolve,0));
  assert.equal(jit.stats().failures,1);assert.equal(jit.stats().mode,'interpreter');
  const before=core.metric(27);jit.step(80000,100);assert(core.metric(27)>before);assert.equal(core.metric(4),0);
  const p=await pair([0x40]);p.a.reset();assert.equal(p.a.jit_register(p.id,1),0);
});

test('self-modified code exits before the stale operation and executes the replacement',async()=>{
  // Guest CS:MOV changes INC AX at offset 2 into DEC AX, then jumps to the loop.
  const p=await pair([0x40],[0x2e,0xc6,0x06,2,0,0x48,0xe9,0xf8,0xfe]);
  for(const core of [p.a,p.b]){setState(core,[256,0,0,0,0,0,0,0],2,0x100);core.step(2);}
  p.a.jit_begin_slice(80000,2);assert.equal(p.a.jit_start(p.id),1);p.run(...layout(p.a));
  assert.equal(p.a.jit_done(),1);assert.equal(p.a.jit_metric(2),1);
  p.b.step(1);assert.deepEqual(state(p.a),state(p.b));
  p.a.jit_dispatch(0);p.b.step(1);assert.equal(state(p.a).r[0],255);assert.deepEqual(state(p.a),state(p.b));
});

test('an IRQ at a translated boundary exits before the next operation; TF never enters',async()=>{
  const p=await pair([0x40]);
  for(const core of [p.a,p.b])setState(core,[42,0,0,0,0x7c00,0,0,0],0x202,1);
  const run=new WebAssembly.Instance(await WebAssembly.compile(compileBlock(p.plan)),{core:{memory:p.a.memory,next:(index,taken)=>{
    p.a.key(0x1d,1);return p.a.jit_next(index,taken);
  }}}).exports.run;
  p.a.jit_begin_slice(80000,2);assert.equal(p.a.jit_start(p.id),1);run(...layout(p.a));
  assert.equal(p.a.jit_done(),1);p.b.step(1);p.b.key(0x1d,1);assert.deepEqual(state(p.a),state(p.b));
  p.a.jit_dispatch(0);p.b.step(1);assert.deepEqual(state(p.a),state(p.b));
  for(const core of [p.a,p.b])setState(core,[0,0,0,0,0x7c00,0,0,0],0x102,1);
  const before=state(p.a);p.a.jit_begin_slice(80000,2);assert.equal(p.a.jit_start(p.id),0);assert.deepEqual(state(p.a),before);
});

test('word translations preserve upper words and cached CPU addresses survive memory growth',async()=>{
  const code=[0x66,0xb8,0xff,0xff,0xef,0xbe,0x90,0x40,0xeb,0xfc];
  const patch=[0x66,0xa3,0,0x80]; // Guest stores EAX through the ordinary memory path.
  const a=await boot(code,patch),b=await boot(code,patch),jit=new WasmJit(a);
  for(let n=0;n<5;n++){
    const done=jit.step(80000,10000);b.step(done);await new Promise(resolve=>setTimeout(resolve,0));
  }
  assert(jit.stats().instructions>0);assert.deepEqual(state(a),state(b));
  a.memory.grow(1);b.memory.grow(1);
  const done=jit.step(80000,1000);b.step(done);assert.deepEqual(state(a),state(b));
  for(const core of [a,b]){
    const [,,p]=layout(core);new DataView(core.memory.buffer).setUint32(p,0x100,true);core.step(1);
  }
  const bytes=core=>Array.from({length:4},(_,i)=>core.peek(0x8000+i));
  assert.deepEqual(bytes(a),bytes(b));assert.deepEqual(bytes(a).slice(2),[0xef,0xbe]);
});

test('reset cancels an old offer without starting concurrent compilation or registering stale code',async()=>{
  const core=await boot([0x90,0x40,0xeb,0xfc]),resolvers=[];
  const jit=new WasmJit(core,{compile:bytes=>new Promise(resolve=>resolvers.push(()=>WebAssembly.compile(bytes).then(resolve)))});
  const turn=()=>new Promise(resolve=>setTimeout(resolve,0));
  jit.step(80000,10000);await turn();assert.equal(resolvers.length,1);
  const old=packet(core)[0];jit.clear();
  const before=core.metric(27);jit.step(80000,10000);await turn();
  assert.equal(resolvers.length,1);assert(core.metric(27)>before);
  await resolvers[0]();for(let n=0;jit.pending;n++){assert(n<100);await turn();}
  assert.equal(jit.stats().compiled,0);assert.equal(core.jit_register(old,1),0);
  jit.step(80000,10000);await turn();assert.equal(resolvers.length,2);
  await resolvers[1]();for(let n=0;jit.pending;n++){assert(n<100);await turn();}
  assert.equal(jit.stats().compiled,1);assert.equal(jit.stats().failures,0);
});
