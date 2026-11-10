import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {compileBlock,WasmJit} from '../web/jit.js';

const module=await WebAssembly.compile(await readFile(new URL('../web/core.wasm',import.meta.url)));
const metrics=[0,1,2,3,5,6,7,9,10,14,15,19,20,23,26,27,28,30];
const packet=core=>new Uint32Array(core.memory.buffer,core.jit_packet()>>>0,core.jit_packet_len()).slice();
const layout=core=>Array.from(new Uint32Array(core.memory.buffer,core.jit_layout()>>>0,4));
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

test('group TEST/NOT/NEG translations match flags and byte/word aliases',async()=>{
  for(const op of [0xf6,0xf7])for(const group of [0,2,3])for(const rm of [0,4,7]){
    const code=[op,0xc0|group<<3|rm,...(group===0?(op===0xf6?[0x81]:[1,0x80]):[])];
    const p=await pair(code);
    for(let n=0;n<100;n++)execute(p,Array.from({length:8},()=>random()&65535),(random()&0xcd5)|2);
  }
});

test('word PUSH/POP preserve SP aliasing, stack bytes and wraparound',async()=>{
  for(let r=0;r<8;r++){
    const p=await pair([0x50+r],[0x58]); // Fallback POP AX checks the value written by generated PUSH.
    for(const sp of [0,2,0x8000,65535]){
      const regs=Array.from({length:8},()=>random()&65535);regs[4]=sp;
      execute(p,regs,0xcd7);
      const expected=regs[r];
      for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,s.flags,0x100);core.step(1);}
      assert.deepEqual(state(p.a),state(p.b));assert.equal(state(p.a).r[0],expected);
    }
    // A conventional POP has a valid two-byte span; offset 0xffff falls back.
    const q=await pair([0x58+r]);
    for(const sp of [0,2,0x8000,65534]){
      const regs=Array.from({length:8},()=>random()&65535);regs[4]=sp;execute(q,regs,0xcd7);
    }
  }
});

test('near CALL/RET translate control flow and retain stack and clock state',async()=>{
  const p=await pair([0xe8,0xfc,0xff],[0xc3]); // Return address 5, destination 1.
  for(const sp of [0,2,0x8000]){
    const regs=[0,0,0,0,sp,0,0,0];execute(p,regs,0x202);
    assert.equal(state(p.a).ip,1);
    // Reference RET checks the return address written by generated CALL.
    for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,s.flags,0x100);core.step(1);}
    assert.equal(state(p.a).ip,5);assert.equal(state(p.a).r[4],sp);assert.deepEqual(state(p.a),state(p.b));
  }
  const q=await pair([0xc3],[0x6a,1,0xe9,0xfc,0xfe]); // PUSH 1; JMP 1 prepares a safe return target.
  for(const core of [q.a,q.b]){setState(core,[0,0,0,0,0x8000,0,0,0],2,0x100);core.step(2);}
  q.a.jit_begin_slice(80000,2);assert.equal(q.a.jit_start(q.id),1);q.run(...layout(q.a));q.b.step(2);
  assert.equal(q.a.jit_done(),2);assert.equal(state(q.a).ip,1);assert.deepEqual(state(q.a),state(q.b));
});

test('memory translations retain effective addresses, aliases, bytes and bus timing',async()=>{
  const forms=[];
  for(let mode=0;mode<3;mode++)for(let rm=0;rm<8;rm++)forms.push({m:mode<<6|rm,
    disp:mode===1?[0xf1]:mode===2?[0xe0,0xff]:rm===6?[0x45,0x23]:[]});
  const cases=[];
  for(const {m,disp} of forms)for(const [op,r,imm] of [[0x01,3,[]],[0x8b,5,[]],[0x87,6,[]],[0xc7,0,[0x81,0x80]]]){
    cases.push({code:[op,m|r<<3,...disp,...imm],m,disp});
  }
  for(let kind=0;kind<8;kind++)for(const wide of [0,1])for(const reverse of [0,2]){
    cases.push({code:[kind*8+wide+reverse,0x9e,0xe0,0xff],m:0x9e,disp:[0xe0,0xff]});
  }
  for(const op of [0x80,0x81,0x83])for(let group=0;group<8;group++){
    cases.push({code:[op,0x86|group<<3,0xe0,0xff,...(op===0x81?[1,0x80]:[0x81])],m:0x86,disp:[0xe0,0xff]});
  }
  for(const op of [0x84,0x85,0x86,0x88,0x89,0x8a,0xc6,0xf6,0xf7]){
    for(const group of op>=0xf6?[0,2,3]:[0])cases.push({code:[op,0x87|group<<3,0xe0,0xff,
      ...(op===0xc6||op>=0xf6&&group===0?(op===0xf7?[1,0x80]:[0x81]):[])],m:0x87,disp:[0xe0,0xff]});
  }
  for(const op of [0xa0,0xa1,0xa2,0xa3])cases.push({code:[op,0x45,0x23],offset:0x2345});
  const patch=[0xb8,0,0x20,0x8e,0xd8,0xb8,0,0x30,0x8e,0xd0,0x90];
  patch.length=0x20;patch.fill(0,11);patch.push(0x89,0x07,0x36,0x89,0x07);
  for(const {code,m,disp,offset} of cases){
    const p=await pair(code,patch);
    for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,2,0x100);core.step(5);}
    for(let n=0;n<8;n++){
      const regs=Array.from({length:8},()=>0x2000+(random()&0x1fff));
      const rm=m&7,mode=m>>6;
      const base=offset??[regs[3]+regs[6],regs[3]+regs[7],regs[5]+regs[6],regs[5]+regs[7],regs[6],regs[7],mode===0?(disp[0]|disp[1]<<8):regs[5],regs[3]][rm];
      const displacement=offset!==undefined||mode===0?0:mode===1?(disp[0]<<24)>>24:((disp[0]|disp[1]<<8)<<16)>>16;
      const at=offset??((base+displacement)&65535),ss=offset===undefined&&(rm===2||rm===3||rm===6&&mode!==0);
      // Seed both segments through reference guest stores; then restore the
      // same registers so a destination alias cannot silently move the write.
      const seed=[random()&65535,0,0,at,0,0,0,0];
      for(const core of [p.a,p.b]){setState(core,seed,2,0x120);core.step(2);}
      execute(p,regs,(random()&0xcd5)|2);
      const physical=(ss?0x30000:0x20000)+at;
      assert.deepEqual(Array.from({length:4},(_,i)=>p.a.peek(physical+i)),Array.from({length:4},(_,i)=>p.b.peek(physical+i)));
    }
  }
});

test('generated memory operations preserve GRCG writes across all four video planes',async()=>{
  const setup=[0xb8,0,0xa8,0x8e,0xd8,0xb0,0xc0,0xe6,0x7c];
  for(const tile of [0xf0,0x0f,0xaa,0x55])setup.push(0xb0,tile,0xe6,0x7e);
  setup.push(0x90);setup.length=0x40;setup.fill(0,26);setup.push(0xb0,0,0xe6,0x7c);
  for(const code of [[0x89,0x07],[0x01,0x07],[0x87,0x07],[0xf7,0x17],[0xf7,0x1f]]){
    const p=await pair(code,setup);
    for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,2,0x100);core.step(13);}
    execute(p,[0x81c3,0,0,3,0x8000,0,0,0],0x807);
    for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,s.flags,0x140);core.step(2);}
    assert.deepEqual(state(p.a),state(p.b));
    for(const plane of [0xa8000,0xb0000,0xb8000,0xe0000]){
      assert.deepEqual(Array.from({length:8},(_,i)=>p.a.peek(plane+i)),Array.from({length:8},(_,i)=>p.b.peek(plane+i)));
    }
  }
});

test('a translated memory store invalidates a later instruction before it executes',async()=>{
  const p=await pair([0x88,0x07],[0xb8,0xc0,0x1f,0x8e,0xd8]); // DS=CS through reference MOV.
  for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,2,0x100);core.step(2);setState(core,[0x48,0,0,4,0x8000,0,0,0],2,1);}
  p.a.jit_begin_slice(80000,3);assert.equal(p.a.jit_start(p.id),1);p.run(...layout(p.a));p.b.step(2);
  assert.equal(p.a.jit_done(),2);assert.equal(p.a.jit_metric(2),1);assert.deepEqual(state(p.a),state(p.b));
  p.a.jit_fallback();p.b.step(1);assert.equal(state(p.a).r[0],0x47);assert.deepEqual(state(p.a),state(p.b));
});

test('Wasm dispatcher falls back after entry invalidation without compiling a stale packet',async()=>{
  const core=await boot([0x90,0x40,0xeb,0xfc],[0x2e,0xc6,0x06,2,0,0x48]);
  const jit=new WasmJit(core);
  for(let n=0;n<8;n++){jit.step(80000,10000);await new Promise(resolve=>setTimeout(resolve,0));}
  assert(jit.stats().instructions>10000);
  const s=state(core);setState(core,s.r,2,0x100);core.step(1);
  setState(core,[0x8000,0,0,0,0x8000,0,0,0],2,1);
  jit.step(80000,100);assert.equal(core.metric(4),0);assert(jit.stats().invalidations>0);
  await new Promise(resolve=>setTimeout(resolve,0));assert.equal(jit.stats().failures,0);
  jit.clear();assert.equal(jit.stats().cached,0);
  for(let slot=0;slot<jit.table.length;slot++)assert.equal(jit.table.get(slot),null);
});

test('Wasm dispatcher preserves unsigned identities across the JavaScript boundary',async()=>{
  const memory=new WebAssembly.Memory({initial:1}),id=0x80000000;
  new Uint32Array(memory.buffer,0,4).set([0x100,0x120,0x124,0x130]);
  new Uint32Array(memory.buffer,32,6).set([id,1,1,1,0x90,0]);
  let offers=0,registered=0;
  const core={memory,metric:()=>66000000,jit_layout:()=>0,jit_begin_slice:()=>{},
    jit_dispatch:()=>offers++===0?id:0,jit_enter:()=>2,jit_fallback:()=>assert.fail('unexpected fallback'),
    jit_done:()=>0,jit_packet:()=>32,jit_packet_len:()=>6,jit_next:()=>0,jit_metric:()=>0,
    jit_register:value=>{registered=value;return 1;},jit_clear:()=>{}};
  const jit=new WasmJit(core);jit.step(80000,100);
  for(let n=0;jit.pending;n++){assert(n<100);await new Promise(resolve=>setTimeout(resolve,0));}
  assert.equal(registered,id);assert(jit.cache.has(id));assert.equal(jit.stats().failures,0);
});

test('predecoded register-change policies preserve the next memory address dependency clocks',async()=>{
  const cases=[
    [[0x43],[0x8a,0x00]],[[0x4b],[0x8a,0x00]],[[0x3b,0xdb],[0x8a,0x00]],
    [[0x83,0xc3,0],[0x8a,0x00]],[[0x89,0xdb],[0x8a,0x00]],[[0x87,0xf3],[0x8a,0x00]],
    [[0x40],[0x67,0x8a,0x00]],[[0x48],[0x67,0x8a,0x00]],[[0xe2,0],[0x67,0x8a,0x01]],
    [[0x51],[0x67,0x8a,0x04,0x24]],[[0xe8,0xfc,0xff],[0x67,0x8a,0x04,0x24]],
    [[0x90],[0x8a,0x00]],[[0x9e],[0x8a,0x00]],[[0xf7,0xd3],[0x8a,0x00]],
  ];
  for(const [instruction,read] of cases){
    const p=await pair(instruction,read);
    for(const value of [0,1,0x1000,0x3000,0xffff]){
      execute(p,[value,value,0,value,0x8000,0,0x1000,0],0x807);
      // The fallback read observes the previous instruction's true GPR-change
      // mask. Identity writes must not invent an address-generation penalty.
      for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,s.flags,0x100);core.step(1);}
      assert.deepEqual(state(p.a),state(p.b));
    }
  }
  const patch=[0x68,0,0x80];patch.length=0x20;patch.fill(0,3);patch.push(0x67,0x8a,0x04,0x24);
  const p=await pair([0x5c],patch);
  for(const core of [p.a,p.b]){setState(core,[0,0,0,0,0x8002,0,0,0],2,0x100);core.step(1);}
  execute(p,[0,0,0,0,0x8000,0,0,0],2); // POP SP reloads the original SP: no change.
  for(const core of [p.a,p.b]){const s=state(core);setState(core,s.r,s.flags,0x120);core.step(1);}
  assert.deepEqual(state(p.a),state(p.b));
});
