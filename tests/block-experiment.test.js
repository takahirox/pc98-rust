// Actual release-Wasm conformance for the browser RAM trace tier.
import {test} from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {compileRamBlock} from '../tools/lib/block-compiler.mjs';
import {BlockJit} from '../tools/lib/block-jit.mjs';
if(!process.env.PC98_BLOCK_WASM)throw Error('Set PC98_BLOCK_WASM to the feature-enabled release Wasm outside web/');
const module=await WebAssembly.compile(await readFile(process.env.PC98_BLOCK_WASM));
const layout=c=>Array.from(new Uint32Array(c.memory.buffer,c.jit_layout()>>>0,4));
function state(c){const[r,f,p]=layout(c),v=new DataView(c.memory.buffer);return {regs:Array.from(new Uint16Array(c.memory.buffer,r,8)),flags:v.getUint16(f,true),ip:v.getUint32(p,true),cs:c.metric(17),metrics:[0,1,2,5,6,7,9,10,14,15,19,20,23,26,27,28,30].map(i=>c.metric(i)),debug:Array.from({length:9},(_,i)=>c.block_debug(i)),fault:c.metric(4)};}
function setState(c,regs,flags=2,ip=1){const[r,f,p]=layout(c),v=new DataView(c.memory.buffer);new Uint16Array(c.memory.buffer,r,8).set(regs);v.setUint16(f,flags,true);v.setUint32(p,ip,true);}
function equal(a,b){assert.deepEqual(state(a),state(b));const frame=c=>Buffer.from(c.memory.buffer,c.frame()>>>0,1024000);assert(frame(a).equals(frame(b)),'rendered frame differs');const ram=c=>Buffer.from(c.memory.buffer,c.block_ram_ptr()>>>0,0xa0000);assert(ram(a).equals(ram(b)),'conventional RAM differs');const pcm=c=>Buffer.from(c.memory.buffer,c.audio_ptr()>>>0,c.audio_len()*4);assert(pcm(a).equals(pcm(b)),'PCM bits differ');}
async function boot(code,patch=[],profile=1){const c=new WebAssembly.Instance(module,{}).exports;c.set_profile(profile);c.set_boot_source(0);const disk=new Uint8Array(1261568);disk.set([0xfa,...code]);disk.set(patch,0x100);const p=c.input_buffer(disk.length)>>>0;new Uint8Array(c.memory.buffer,p,disk.length).set(disk);assert.equal(c.insert(0,0),1);c.reset();for(let n=0;c.metric(17)!==0x1fc0||c.metric(18)!==1;n++){assert(n<2000);assert.equal(c.metric(4),0);c.step(1);}return c;}
async function translate(a,b,ip=1,expectedMeta=1){let id=0;for(let n=0;!id;n++){assert(n<64);for(const c of[a,b]){const s=state(c);setState(c,s.regs,s.flags,ip);}a.jit_devices_begin();a.block_begin_slice(80000,1);id=a.jit_dispatch(1);a.jit_devices_end();b.step(a.jit_done());}const packet=new Uint32Array(a.memory.buffer,a.jit_packet()>>>0,a.jit_packet_len()).slice(),meta=new Uint32Array(a.memory.buffer,a.block_packet()>>>0,a.block_packet_len()).slice();assert.equal(meta[0],expectedMeta|4,'probe is not the expected direct RAM plan');const bytes=compileRamBlock(packet,meta);assert(WebAssembly.validate(bytes));const run=new WebAssembly.Instance(await WebAssembly.compile(bytes),{core:{memory:a.memory,finish:a.block_finish}}).exports.run;assert.equal(a.jit_register(id,1),1);equal(a,b);return {a,b,id,run,packet,meta,ip};}
async function pair(code,patch=[],initial=null,expectedMeta=1){const a=await boot(code,patch),b=await boot(code,patch);if(initial)for(const c of[a,b])setState(c,initial);return translate(a,b,1,expectedMeta);}

function execute(p,regs,flags=2,cap=100,clocks=80000){for(const c of[p.a,p.b])setState(c,regs,flags,p.ip);p.a.jit_devices_begin();p.a.block_begin_slice(clocks,cap);if(p.a.block_start(p.id))p.run(...layout(p.a));else p.a.jit_fallback();p.a.jit_devices_end();const done=p.a.jit_done();p.b.step(done);equal(p.a,p.b);return done;}

test('RAM loop executes loads, stores, arithmetic and a direct back edge without instruction callbacks',async()=>{
 const p=await pair([0x8b,0x07,0x40,0x89,0x07,0xeb,0xf9]); // MOV AX,[BX]; INC AX; MOV [BX],AX; JMP start
 const done=execute(p,[0,0,0,0x8000,0,0,0,0],2,40);
 assert(done>p.packet[1]&&done<=40);assert.equal(p.a.peek(0x8000),Math.floor((done+1)/4));assert.equal(p.a.block_metric(0),1);assert.equal(p.a.block_metric(1),done);
});

test('RAM block respects partial work/event exits and exact unaligned/dependency timing',async()=>{
 for(const bx of [0x8000,0x8001,0x8002,0x8003])for(const cap of [1,2,3,4,5,19]){
  const p=await pair([0x43,0x8b,0x07,0x83,0xc0,1,0x89,0x07,0xeb,0xf6]);
  execute(p,[0,0,0,bx,0,0,0,0],0x403,cap);
 }
 const p=await pair([0x40,0x48,0xeb,0xfc]);execute(p,[65535,0,0,0,0,0,0,0],3,100,1);
});

test('real-mode high-memory traces stop at the A20 gate and recheck changed code',async()=>{
 const setup=[0xe6,0xf2,0xea,0x20,0,0xff,0xff]; // Enable A20 and enter FFFF:0020.
 const a=await boot(setup),b=await boot(setup),j=new BlockJit(a);
 const cores=[a,b];
 const ram=c=>new Uint8Array(c.memory.buffer,c.block_ram_ptr()>>>0,0x110000);
 for(const c of cores){
  ram(c).set([0x40,0xeb,0xfd],0x100010); // High loop increments AX.
  ram(c).set([0x41,0xeb,0xfd],0x10); // Wrapped loop increments CX.
  ram(c).set([0xb0,3,0xe6,0xf6],0x1000f0); // Disable A20 while still in high RAM.
  ram(c).set([0xe6,0xf2],0xf0); // Re-enable A20 from the wrapped address.
  c.step(2);
  assert.equal(state(c).ip,0x20);
 }
 async function compare(){
  const done=j.step(8000,120);
  b.step(done);
  while(j.pending)await new Promise(resolve=>setTimeout(resolve,0));
  assert(!j.failed,j.compileError);
  equal(a,b);
  for(const c of cores)c.audio_clear();
 }
 function runGateProgram(units){
  for(const c of cores){
   const s=state(c);
   setState(c,s.regs,s.flags,0x100);
   c.step(units);
   const after=state(c);
   setState(c,after.regs,after.flags,0x20);
  }
 }
 for(let run=0;run<12;run++)await compare();
 const translated=j.stats().blockInstructions;
 assert(translated>0);

 runGateProgram(2);
 const cx=state(a).regs[1];
 await compare();
 assert(state(a).regs[1]>cx);
 assert.equal(j.stats().blockInstructions,translated);

 runGateProgram(1);
 const dx=state(a).regs[2],invalidations=a.jit_metric(2);
 for(const c of cores)ram(c)[0x100010]=0x42;
 await compare();
 assert(state(a).regs[2]>dx);
 assert(a.jit_metric(2)>invalidations);
});

test('a direct high-memory return stays in RAM and falls back at the ROM boundary',async()=>{
 const setup=[0xe6,0xf2,0xea,0x20,0,0xff,0xff];
 const regs=[0x120,0,0,0,0x8000,0,0,0];
 async function highPair(code,stack=[]){
  const a=await boot(setup),b=await boot(setup);
  for(const c of [a,b]){
   const ram=new Uint8Array(c.memory.buffer,c.block_ram_ptr()>>>0,0x110000);
   ram.set(code,0x100010);ram.set(stack,0x8000);
   c.step(2);
   setState(c,regs,2,0x20);
  }
  return translate(a,b,0x20);
 }
 const trace=await highPair([0xe8,2,0,0xeb,0xfb,0x40,0xc3]); // CALL helper; JMP start; helper: INC AX; RET.
 assert.equal(execute(trace,regs,2,4),4);
 const returned=state(trace.a);
 assert.equal(returned.ip,0x20);
 assert.equal(returned.regs[0],0x121);
 assert.equal(returned.regs[4],0x8000);
 assert.equal(trace.a.block_metric(1),4);

 const stack=Uint8Array.from({length:128},(_,i)=>i%2 ? 0 : 0x0f);
 const rom=await highPair([0xc3],stack);
 const fallback=rom.a.block_metric(2);
 assert.equal(execute(rom,regs,2,2),1);
 assert.equal(state(rom.a).ip,0x0f); // FFFF:000F points to ROM.
 assert.equal(rom.a.block_metric(2),fallback+1);
});

test('direct traces read visible segment selectors and push the four legacy selectors',async()=>{
 const setup=[0xb8,0x34,0x12,0x8e,0xd8,0xb8,0x78,0x56,0x8e,0xc0,0x31,0xc0,0x8e,0xd0,0x90];
 const code=[...setup,0x06,0x8c,0xc3,0x0e,0x16,0x1e,0x8c,0xd9,0xeb,0xf6];
 const a=await boot(code),b=await boot(code);
 const regs=[0,0,0,0,0x8000,0,0,0];
 for(const c of [a,b]){
  c.step(7);
  assert.equal(state(c).ip,1+setup.length);
  setState(c,regs,2,1+setup.length);
 }
 const p=await translate(a,b,1+setup.length);
 assert.equal(execute(p,regs,2,6),6);
 const {regs:after,cs}=state(a);
 assert.equal(after[1],0x1234);
 assert.equal(after[3],0x5678);
 assert.equal(after[4],0x7ff8);
 const stack=new DataView(a.memory.buffer,(a.block_ram_ptr()>>>0)+0x7ff8,8);
 assert.deepEqual([0,2,4,6].map(offset=>stack.getUint16(offset,true)),[0x1234,0,cs,0x5678]);
 assert.equal(a.block_metric(1),6);
});

test('linked traces retire once per event and revalidate code changed by an earlier trace',async()=>{
 // The store is in the first trace; its eventual target lies beyond that trace.
 const body=[0x2e,0xc6,0x07,0x48,...Array(40).fill(0x40)];
 const code=[...body,0xeb,(-body.length-2)&255];
 const a=await boot(code),b=await boot(code),j=new BlockJit(a);
 for(const c of[a,b])setState(c,[0,0,0,0x8000,0,0,0,0]);
 for(let n=0;n<50;n++){
  const done=j.step(8000,1000);b.step(done);
  while(j.pending)await new Promise(r=>setTimeout(r,0));
  assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();
 }
 const stats=j.stats();assert(stats.blockTraces>stats.blockCalls*1.2,'multiple traces must share a completion');
 for(const [clocks,cap] of[[1,1000],[20,1000],...[1,15,16,17,31,50,1000].map(cap=>[8000,cap])]){
  for(const c of[a,b])setState(c,[0,0,0,0x25,0,0,0,0],2,1);
  const done=j.step(clocks,cap);b.step(done);
  while(j.pending)await new Promise(r=>setTimeout(r,0));
  assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();
 }
 assert.equal(a.peek(0x1fc25),0x48);
});

test('generated RAM byte aliases, carries, identities and memory dependencies match the interpreter',async()=>{
 const cases=[[0x04,1,0x14,0,0x1c,0,0x86,0xe0,0x9e,0x9f], [0x8a,0x27,0x00,0x27,0x80,0x37,0x81], [0x89,0xdb,0x3b,0xdb,0x8b,0x07], [0xf7,0x1f,0xff,0x07,0x83,0x3f,0], [0xc7,0x07,0xff,0x7f,0x01,0x07,0x8b,0x07]];
 for(const code of cases){const p=await pair([...code,0xeb,(-code.length-2)&255]);for(const flags of [2,3,0xcd7])execute(p,[0x80ff,2,0,0x8001,0,0,0,0],flags,17);}
});

test('a self-modifying store exits before the store and resumes through the original instruction',async()=>{
 const p=await pair([0x40,0x2e,0xc6,0x06,9,0,0x48,0x90,0x40,0xeb,0xf5]);
 assert.equal(execute(p,[0,0,0,0,0,0,0,0],2,5),1);assert.equal(p.a.peek(0x1fc09),0x40);
 for(const c of[p.a,p.b]){const s=state(c);setState(c,s.regs,s.flags,2);}
 p.a.jit_devices_begin();p.a.block_begin_slice(80000,1);p.a.jit_dispatch(0);p.a.jit_devices_end();p.b.step(p.a.jit_done());equal(p.a,p.b);assert.equal(p.a.peek(0x1fc09),0x48);
});

test('guarded MOV preserves mapped word transfers and rejects a segment-crossing word',async()=>{
 for(const opcode of [0x89,0x8b])for(const bx of [0xffff,0x8000]){
  const p=await pair([opcode,0x07,0x40,0xeb,0xfb]);
  const mapped=bx===0x8000;
  if(mapped)for(const c of [p.a,p.b]){
   const ptr=c.block_ram_ptr()>>>0;
   const setup=[0xb8,0,0xa8,0x8e,0xd8,0xb8,0x45,0x23,0xa3,0,0x80]; // DS=A800; seed the mapped word.
   new Uint8Array(c.memory.buffer,ptr+0x1fd00,setup.length).set(setup);
   const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(4);
  }
  const translated=p.a.jit_metric(1);
  execute(p,[0x9876,0,0,bx,0,0,0,0],2,1);
  assert(p.a.block_metric(2)>0);
  assert.equal(p.a.jit_metric(1)-translated,Number(mapped));
  if(mapped&&opcode===0x89){
   assert.equal(p.a.peek(0xb0000),0x76);assert.equal(p.a.peek(0xb0001),0x98);
  }
  if(mapped&&opcode===0x8b)assert.equal(state(p.a).regs[0],0x2345);
 }
});

test('pending IRQ, trap and zero budgets reject admission without a fetch',async()=>{
 const p=await pair([0x40,0x48,0xeb,0xfc]);
 for(const [flags,clocks,cap,key]of [[0x102,80000,4,false],[2,0,4,false],[2,80000,0,false],[0x202,80000,4,true]]){
  for(const c of[p.a,p.b]){setState(c,[0,0,0,0,0,0,0,0],flags);if(key)c.key(0x1d,1);}
  const before=state(p.a);p.a.jit_devices_begin();p.a.block_begin_slice(clocks,cap);assert.equal(p.a.block_start(p.id),0);p.a.jit_devices_end();assert.deepEqual(state(p.a),before);equal(p.a,p.b);
 }
});

test('the experimental owner runs mixed direct and guarded plans with exact guest time',async()=>{
 const code=[0x8b,0x07,0x40,0x89,0x07,...Array(13).fill(0x40),0xd1,0xe0,0x8d,0x3f,0xeb,0xe8]; // Shift coverage plus a later timing-mode transition.
 const a=await boot(code),b=await boot(code),j=new BlockJit(a);
 for(const c of[a,b])setState(c,[0,0,0,0x8000,0,0,0,0]);
 for(let n=0;n<40;n++){const done=j.step(8000,1000);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();}
 assert(j.stats().blockInstructions>0);
 // Cached timing retains the guarded tier after invalidating direct plans.
 for(const c of[a,b]){
  new Uint8Array(c.memory.buffer,(c.block_ram_ptr()>>>0)+0x1fd00,12).set([0x0f,0x20,0xc0,0x66,0x25,0xff,0xff,0xff,0x9f,0x0f,0x22,0xc0]);
  const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(3);assert.equal(c.block_debug(0)&0x40000000,0);assert.equal(c.metric(4),0);setState(c,s.regs,s.flags,1);
 }
 for(let n=0;n<20;n++){const done=j.step(8000,1000);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();}
 assert(a.jit_metric(1)>j.stats().blockInstructions);
});

// Literal arithmetic/branch expectations complement differential timing checks.
test('carry, overflow and conditional back edges have independent architectural results',async()=>{
 const p=await pair([0x04,1,0x14,0,0x70,0xfa]); // ADD AL,1; ADC AL,0; JO start
 assert.equal(execute(p,[0x7f,0,0,0,0,0,0,0],2,1),1);
 assert.equal(state(p.a).regs[0],0x80);assert.equal(state(p.a).flags&0x8d5,0x890);
 const q=await pair([0x04,1,0x14,0,0x72,0xfa]);
 assert.equal(execute(q,[0xff,0,0,0,0,0,0,0],2,2),2);
 assert.equal(state(q.a).regs[0],1);assert.equal(state(q.a).flags&0x8d5,0);
 const loop=await pair([0x40,0xe2,0xfd]); // INC AX; LOOP start
 assert.equal(execute(loop,[0,3,0,0,0,0,0,0],2,6),6);
 assert.equal(state(loop.a).regs[0],3);assert.equal(state(loop.a).regs[1],0);assert.equal(state(loop.a).ip,4);
});

test('mutating a later instruction invalidates the whole entry before any instruction retires',async()=>{
 const p=await pair([0x40,0x48,0xeb,0xfc]);
 for(const c of[p.a,p.b]){const ptr=c.block_ram_ptr()>>>0;new Uint8Array(c.memory.buffer)[ptr+0x1fc02]=0x40;setState(c,[7,0,0,0,0,0,0,0]);}
 const before=state(p.a);p.a.jit_devices_begin();p.a.block_begin_slice(80000,4);assert.equal(p.a.block_start(p.id),0);p.a.jit_devices_end();assert.deepEqual(state(p.a),before);equal(p.a,p.b);assert.equal(p.a.jit_metric(2),1);
});

test('all RAM ALU directions and immediate groups preserve boundary carries and flags',async()=>{
 for(let kind=0;kind<8;kind++)for(const wide of [0,1])for(const direction of [0,2]){
  const code=[kind*8+wide+direction,0x07];
  const p=await pair([...code,0xeb,0xfc]);
  for(const value of [0,1,0x7f,0x80,0xff,0x7fff,0x8000,0xffff]){
   for(const c of[p.a,p.b])new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+0x8003,value,true);
   execute(p,[value^0xffff,2,0,0x8003,0,0,0,0],0xcd7,1);
  }
 }
 for(let kind=0;kind<8;kind++){
  const p=await pair([0x83,0x07|kind<<3,0xff,0xeb,0xfb]);
  for(const value of [0,0x7fff,0xffff]){
   for(const c of[p.a,p.b])new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+0x8001,value,true);
   execute(p,[0,0,0,0x8001,0,0,0,0],0xcd7,1);
  }
 }
});

test('every conditional and LOOP outcome preserves direct-branch timing',async()=>{
 for(const op of [...Array.from({length:16},(_,i)=>0x70+i),0xe0,0xe1,0xe2,0xe3]){
  const p=await pair([op,0xfe]);
  for(let bits=0;bits<32;bits++)for(const cx of [0,1,2,0xffff]){
   const flags=2|(bits&1)|((bits&2)<<1)|((bits&4)<<4)|((bits&8)<<4)|((bits&16)<<7);
   execute(p,[0,cx,0,0,0,0,0,0],flags,1);
  }
 }
});

test('segment overrides and all 16-bit effective addresses retain dynamic bases and fetch alignment',async()=>{
 for(const prefix of [0x26,0x2e,0x36,0x3e,0x64,0x65])for(const rm of [0,1,2,3,4,5,6,7]){
  const p=await pair([prefix,0x8b,0x40|rm,0xff,0xeb,0xfa],[],[0,0,0,0x8003,0,0x8001,0x10,0x20]);
  execute(p,[0,0,0,0x8003,0,0x8001,0x10,0x20],2,9);
 }
});

test('direct RAM locals preserve upper register words and survive Wasm memory growth',async()=>{
 const p=await pair([0x8b,0x07,0x40,0x89,0x07,0xeb,0xf9]);
 for(const c of[p.a,p.b]){
  new Uint8Array(c.memory.buffer,(c.block_ram_ptr()>>>0)+0x1fd00,10).set([0x66,0xb8,0xdc,0xfe,0x34,0x12,0x66,0xa3,0,0x80]);
  setState(c,state(c).regs,2,0x100);c.step(1);c.memory.grow(1);
 }
 execute(p,[0,0,0,0x8000,0,0,0,0],2,4);
 for(const c of[p.a,p.b]){const s=state(c);setState(c,s.regs,s.flags,0x106);c.step(1);}
 equal(p.a,p.b);assert.deepEqual([p.a.peek(0x8002),p.a.peek(0x8003)],[0x34,0x12]);
});

test('reset cancels an experimental compilation without registering stale RAM code',async()=>{
 const c=await boot([0x8b,0x07,0x40,0xeb,0xfb]),resolvers=[],j=new BlockJit(c,{compile:bytes=>new Promise(resolve=>resolvers.push(()=>WebAssembly.compile(bytes).then(resolve)))});
 setState(c,[0,0,0,0x8000,0,0,0,0]);const turn=()=>new Promise(r=>setTimeout(r,0));
 j.step(80000,10000);await turn();assert.equal(resolvers.length,1);
 const old=new Uint32Array(c.memory.buffer,c.jit_packet()>>>0,1)[0];j.clear();
 j.step(80000,10000);await turn();assert.equal(resolvers.length,1);
 await resolvers[0]();for(let n=0;j.pending;n++){assert(n<100);await turn();}
 assert.equal(j.stats().compiled,0);assert.equal(c.jit_register(old,1),0);
 j.step(80000,10000);await turn();assert.equal(resolvers.length,2);
 await resolvers[1]();for(let n=0;j.pending;n++){assert(n<100);await turn();}
 assert.equal(j.stats().compiled,1);assert.equal(j.stats().failures,0);
});


test('direct PUSH/POP preserve stack aliases, word wrapping and exact early-exit timing',async()=>{
 for(let op=0x50;op<=0x5f;op++){
  for(const sp of [0x8000,0x8001,0,1,0xfffe,0xffff]){
   const p=await pair([op,0xeb,0xfd],[],[0,0,0,0,0x8000,0,0,0]);
   for(const c of[p.a,p.b]){const off=op<0x58?(sp-2)&0xffff:sp;if(off<0xffff)new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+off,0x9876,true);}
   execute(p,[0x1234,2,3,4,sp,6,7,8],3,1);
   if(sp===0x8000&&op===0x54){assert.equal(state(p.a).regs[4],0x7ffe);assert.equal(p.a.peek(0x7fff),0x80);}
   if(sp===0x8000&&op===0x5c)assert.equal(state(p.a).regs[4],0x9876);
  }
 }
});

test('direct POP SP identity preserves the actual register-change mask',async()=>{
 const p=await pair([0x5c,0xeb,0xfd],[],[0,0,0,0,0x8000,0,0,0]);
 for(const c of[p.a,p.b])new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+0x8000,0x8000,true);
 execute(p,[0,0,0,0,0x8000,0,0,0],2,1);assert.equal(state(p.a).regs[4],0x8000);assert.equal(p.a.block_debug(1),0);
});

test('direct near CALL/RET preserve return words and decline a mapped return before SP/IP effects',async()=>{
 const call=await pair([0xe8,3,0,0x90,0xeb,0xfa,0xc3],[],[0,0,0,0,0x8000,0,0,0]);
 execute(call,[0,0,0,0,0x8000,0,0,0],2,1);assert.equal(state(call.a).ip,7);assert.equal(state(call.a).regs[4],0x7ffe);assert.equal(call.a.peek(0x7ffe),4);
 const a=await boot([0x90]),b=await boot([0x90]);
 for(const c of[a,b]){
  const ptr=c.block_ram_ptr()>>>0;new Uint8Array(c.memory.buffer,ptr+0x98100,1)[0]=0xc3;
  new Uint8Array(c.memory.buffer,ptr+0x1fd00,5).set([0xea,0,1,0,0x98]);
  setState(c,[0,0,0,0,0x8000,0,0,0],2,0x100);c.step(1);
 }
 const ret=await translate(a,b,0x100);
 for(const target of [0x100,0x9000]){
  for(const c of[a,b])new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+0x8000,target,true);
  execute(ret,[0,0,0,0,0x8000,0,0,0],2,1);assert.equal(state(a).ip,target);assert.equal(state(a).regs[4],0x8002);
 }
 assert(a.block_metric(2)>0);
});


test('direct shifts and rotations preserve byte aliases, RAM, carries, count masking and partial exits',async()=>{
 for(const wide of [0,1])for(let kind=0;kind<8;kind++)for(const memory of [false,true]){
  for(const count of [0,1,2,7,8,9,15,16,17,31,32,33,255]){
   if(count!==1&&(kind===2||kind===3))continue; // Unsupported immediate carry rotates stay interpreted.
   const code=count===1?[0xd0+wide,(kind<<3)|(memory?7:0xc4)]:[0xc0+wide,(kind<<3)|(memory?7:0xc4),count];
   const p=await pair([...code,0xeb,(-code.length-2)&255],[],[0,0,0,0x8001,0,0,0,0]);
   for(const value of [0,0x80ff,0xffff]){
    for(const c of[p.a,p.b])new DataView(c.memory.buffer).setUint16((c.block_ram_ptr()>>>0)+0x8001,value,true);
    execute(p,[value,0,0,0x8001,value,0,0,0],0xcd7,1);
   }
  }
 }
});


test('direct jump traces retain register locals across noncontiguous spans and invalidate later spans',async()=>{
 const code=[0x40,0xeb,5,...Array(5).fill(0x90),0x43,0xeb,0xf5];
 for(const cap of [1,2,3,4,5,17]){
  const p=await pair(code);assert.deepEqual(Array.from({length:p.packet[1]},(_,i)=>p.packet[2+i*4]),[1,2,9,10]);
  const done=execute(p,[0,0,0,0,0,0,0,0],2,cap);assert(done<=cap);
 }
 const p=await pair(code);
 for(const c of[p.a,p.b])new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x1fc09]=0x4b;
 const before=state(p.a);p.a.jit_devices_begin();p.a.block_begin_slice(80000,10);assert.equal(p.a.block_start(p.id),0);p.a.jit_devices_end();assert.deepEqual(state(p.a),before);equal(p.a,p.b);assert.equal(p.a.jit_metric(2),1);
});

test('a direct CALL/callee/RET/caller trace commits in one completion',async()=>{
 const p=await pair([0xe8,3,0,0x90,0xeb,0xfa,0x40,0xc3],[],[0,0,0,0,0x8000,0,0,0]);
 assert.equal(p.packet[1],5);
 // Start immediately after an actual audio deadline so this control exercises
 // the full trace; the separate prefix tests deliberately stop before events.
 const samples=p.a.audio_len();
 for(let n=0;p.a.audio_len()===samples;n++){
  assert(n<2000);for(const c of[p.a,p.b]){new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x1fd00]=0x90;const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(1);}
 }
 assert.equal(execute(p,[7,0,0,0,0x8000,0,0,0],2,5),5);
 assert.equal(state(p.a).regs[0],8);assert.equal(state(p.a).regs[4],0x8000);assert.equal(state(p.a).ip,1);assert.equal(p.a.block_metric(0),1);
});

test('direct LEA preserves wrapped addresses, flags and dependencies without reading mapped memory',async()=>{
 for(const prefix of [[],[0x26],[0x36],[0x3e],[0x64],[0x65]])for(const mode of [0,1,2])for(let rm=0;rm<8;rm++){
  const disp=mode===1?[0xff]:mode===2||rm===6?[0xff,0xff]:[];
  const code=[...prefix,0x8d,(mode<<6)|0x18|rm,...disp];
  const p=await pair([...code,0xeb,(-code.length-2)&255]);
  for(const c of[p.a,p.b]){
   new Uint8Array(c.memory.buffer,(c.block_ram_ptr()>>>0)+0x1fd00,5).set([0xb8,0,0xa8,0x8e,0xd8]);
   const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(2);
  }
  execute(p,[0,3,0,0xffff,0,0xffff,0xffff,0xffff],0xcd7,3);
 }
 const p=await pair([0x8d,0x1e,0xff,0xff,0xeb,0xfa]);execute(p,[0,0,0,0,0,0,0,0],3,1);assert.equal(state(p.a).regs[3],0xffff);assert.equal(state(p.a).flags,3);
});

test('a traced write to later code or a gap exits before memory effects',async()=>{
 for(const target of [11,13]){
  const p=await pair([0x40,0x2e,0xc6,0x06,target,0,0x48,0xeb,3,0x90,0x90,0x90,0x40,0xeb,0xf1]);
  const before=p.a.peek(0x1fc00+target);assert.equal(execute(p,[0,0,0,0,0,0,0,0],2,10),1);assert.equal(p.a.peek(0x1fc00+target),before);
  p.a.jit_devices_begin();p.a.block_begin_slice(80000,1);p.a.jit_fallback();p.a.jit_devices_end();p.b.step(1);equal(p.a,p.b);assert.equal(p.a.peek(0x1fc00+target),0x48);
 }
});


test('a changed predicted return commits the actual RET and skips the predicted caller span',async()=>{
 const p=await pair([0xe8,5,0,0x40,0xeb,0xfa,0x90,0x90,0x5b,0xbb,16,0,0x53,0xc3,0x90,0x42,0xeb,0xee],[],[0,0,0,0,0x8000,0,0,0]);
 const samples=p.a.audio_len();
 for(let n=0;p.a.audio_len()===samples;n++){assert(n<2000);for(const c of[p.a,p.b]){new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x1fd00]=0x90;const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(1);}}
 assert.equal(execute(p,[7,0,0,0,0x8000,0,0,0],2,100),5);
 assert.equal(state(p.a).ip,16);assert.equal(state(p.a).regs[0],7);assert.equal(state(p.a).regs[2],0);assert.equal(state(p.a).regs[4],0x8000);
});

test('sparse register reloads preserve untouched and future-write registers on partial exits',async()=>{
 const p=await pair([0xba,0x34,0x12,0xbe,0x78,0x56,0xeb,0xf8]);
 execute(p,[1,2,3,4,0x8000,6,7,8],3,1);assert.equal(state(p.a).regs[2],0x1234);assert.equal(state(p.a).regs[6],7);
 const q=await pair([0x2e,0xc6,0x06,1,0,0x2e,0xbe,0x34,0x12,0xeb,0xf5]);
 execute(q,[1,2,3,4,0x8000,6,7,8],3,1);assert.equal(state(q.a).regs[6],7);assert(q.a.block_metric(2)>0);
});

test('a forward conditional follows only its decoded fallthrough and exits on a taken branch',async()=>{
 // CMP AX,0; JNZ target; INC BX; JMP entry; target: INC DX; JMP entry.
 for(const ax of [0,1])for(const cap of [1,2,3,4,19]){
  const p=await pair([0x3d,0,0,0x75,3,0x43,0xeb,0xf8,0x42,0xeb,0xf5]);
  assert.equal(p.packet[1],4);assert.deepEqual(Array.from({length:4},(_,i)=>p.packet[2+i*4]),[1,4,6,7]);
  const done=execute(p,[ax,2,3,4,0x8000,6,7,8],3,cap);
  assert.equal(state(p.a).regs[2],3);
  if(ax&&cap>=2){assert.equal(done,2);assert.equal(state(p.a).ip,9);assert.equal(state(p.a).regs[3],4);}
  if(!ax&&cap>=3)assert(state(p.a).regs[3]>4);
 }
});


test('a taken forward branch leaves fallthrough RAM stores unexecuted',async()=>{
 const p=await pair([0x3d,0,0,0x75,4,0x89,0x07,0xeb,0xf7,0x42,0xeb,0xf4]);
 for(const c of[p.a,p.b])new Uint8Array(c.memory.buffer,(c.block_ram_ptr()>>>0)+0x8000,2).set([0xaa,0xbb]);
 assert.equal(execute(p,[1,2,3,0x8000,0,6,7,8],3,100),2);
 assert.equal(state(p.a).ip,10);assert.equal(p.a.peek(0x8000),0xaa);assert.equal(p.a.peek(0x8001),0xbb);
 execute(p,[0,2,3,0x8000,0,6,7,8],3,3);assert.equal(p.a.peek(0x8000),0);assert.equal(p.a.peek(0x8001),0);
});


test('hot native interrupt fallback preserves work deadlines and rechecks changed vectors and code',async()=>{
 const a=await boot([0xcd,0x30,0xeb,0xfc],[0x40,0xcf]),b=await boot([0xcd,0x30,0xeb,0xfc],[0x40,0xcf]),j=new BlockJit(a);
 for(const c of[a,b]){setState(c,[7,0,0,0,0x8001,0,0,0],3);new Uint8Array(c.memory.buffer).set([0,1,0xc0,0x1f],(c.block_ram_ptr()>>>0)+0xc0);}
 async function step(clocks,cap){const done=j.step(clocks,cap);assert(done>0&&done<=cap);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();}
 for(let n=0;n<12;n++)await step(8000,1000);
 assert(state(a).regs[0]>7);assert.equal(state(a).fault,0);
 for(const clocks of [1,20,80000])for(const cap of [1,2,3,4,17])await step(clocks,cap);
 // The hot hint must consult the current vector and opcode on every unit.
 for(const c of[a,b])new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0xc0]=1;
 for(let n=0;n<8;n++)await step(8000,1000);
 for(const c of[a,b])new Uint8Array(c.memory.buffer).set([0x40,0x90],(c.block_ram_ptr()>>>0)+0x1fc01);
 for(let n=0;n<8;n++)await step(8000,1000);
});

test('hot keyboard polling reads current BIOS groups and keeps literal interrupt/OUT/IRET boundaries',async()=>{
 const code=[0xb4,4,0xb0,3,0xcd,0x18,0xeb,0xf8],a=await boot(code),b=await boot(code),j=new BlockJit(a);
 for(const c of[a,b]){setState(c,[0,0,0,0,0x8001,0,0,0],3);new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x52d]=0xa5;}
 async function step(clocks,cap){const done=j.step(clocks,cap);assert(done>0&&done<=cap);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();}
 for(let n=0;n<12;n++)await step(8000,1000);
 for(const clocks of [1,20,80000])for(const cap of [1,2,3,4,17])await step(clocks,cap);
 for(const group of [3,15,16,255])for(const value of [0xa5,0x80]){
  for(let n=0;a.metric(17)!==0x1fc0;n++){assert(n<16);await step(80000,1);}
  for(const c of[a,b]){const s=state(c);s.regs[0]=0x0400|group;setState(c,s.regs,3,5);if(group<16)new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x52a+group]=value;}
  await step(80000,1);assert.equal(a.metric(17),0xf000);assert.equal(a.metric(18),0x1060);
  await step(80000,1);assert.equal(a.metric(18),0x1062);assert.equal(state(a).regs[0],((group<16?value:0)<<8)|group);
  await step(80000,1);assert.equal(a.metric(17),0x1fc0);assert.equal(a.metric(18),7);assert.equal(state(a).regs[4],0x8001);assert.equal(state(a).flags,3);
 }
});

test('reference interrupt words retain odd and wrapping real-mode stack frames in release Wasm',async()=>{
 for(const sp of [0,2,4,0x8000,0x8001,0xfffe]){
  const c=await boot([0xb8,0,0x30,0x8e,0xd0,0xbc,sp&255,sp>>8,0xcd,0x30,0x90,0xeb,0xfd],[0xcf]);
  const ram=c.block_ram_ptr()>>>0;new Uint8Array(c.memory.buffer).set([0,1,0xc0,0x1f],ram+0xc0);
  c.step(3);let s=state(c);setState(c,s.regs,3,s.ip);assert.equal(s.ip,9);
  c.step(1);s=state(c);assert.equal(s.cs,0x1fc0);assert.equal(s.ip,0x100);assert.equal(s.regs[4],(sp-6)&65535);
  for(const [delta,value] of [[2,3],[4,0x1fc0],[6,11]]){const at=0x30000+((sp-delta)&65535);assert.equal(c.peek(at)|(c.peek(at+1)<<8),value);}
  c.step(1);s=state(c);assert.equal(s.ip,11);assert.equal(s.regs[4],sp);assert.equal(s.flags,3);assert.equal(s.fault,0);
 }
});

test('reference V30 word operands wrap their segment offset before physical bus decoding',async()=>{
 const c=await boot([0xb8,0,0x30,0x8e,0xd8,0xa1,0xff,0xff,0xa3,0xff,0xff,0xeb,0xfe],[],0),ram=c.block_ram_ptr()>>>0;
 const mem=new Uint8Array(c.memory.buffer);mem[ram+0x30000]=0xaa;mem[ram+0x3ffff]=0x55;mem[ram+0x40000]=0xcc;
 c.step(3);let s=state(c);assert.equal(s.regs[0],0xaa55);s.regs[0]=0x1234;setState(c,s.regs,s.flags,s.ip);
 c.step(1);assert.equal(c.peek(0x3ffff),0x34);assert.equal(c.peek(0x30000),0x12);assert.equal(c.peek(0x40000),0xcc);assert.equal(c.metric(4),0);
});


test('a direct keyboard query retires INT, ROM OUT and IRET with the original frame and clocks',async()=>{
 const code=[0xb4,4,0xcd,0x18,0xeb,0xfa];
 const a=await boot(code),b=await boot(code);
 for(const c of[a,b])setState(c,[0x0403,0,0,0,0x8000,0,0,0]);
 const p=await translate(a,b,1,3);
 for(const sp of[0x8000,0x8001,0x8002,0x8003])for(const cap of[1,2,3,4,5,6,19,40]){
  for(const c of[a,b])new Uint8Array(c.memory.buffer,c.block_ram_ptr()>>>0,0xa0000)[0x52d]=0xa5;
  const before=a.metric(26);
  const done=execute(p,[0x0403,0,0,0,sp,0,0,0],0xcd7,cap);
  if(sp===0x8000&&cap===4){assert.equal(a.metric(26)-before,117);assert.equal(a.block_debug(2),15);assert.equal(a.block_debug(3),16);assert.equal(a.block_debug(4),1);}
  assert(done<=cap);
  if(done>=4){assert([1,3,5].includes(state(a).ip));assert.equal(state(a).regs[0],state(a).ip===3?0x0403:0xa503);assert.equal(state(a).regs[4],sp);
   const v=new DataView(a.memory.buffer,(a.block_ram_ptr()>>>0)+sp-6,6);
   assert.equal(v.getUint16(0,true),5);assert.equal(v.getUint16(2,true),0x1fc0);assert.equal(v.getUint16(4,true),0xcd7);
  }
 }
 for(const group of[15,16,255]){
  // Fresh cores leave room before the next audio event for all three units.
  const q=await pair(code,[],[0x0400|group,0,0,0,0x8000,0,0,0],3);
  for(const c of[q.a,q.b])new Uint8Array(c.memory.buffer)[(c.block_ram_ptr()>>>0)+0x539]=0x7f;
  assert.equal(execute(q,[0x0400|group,0,0,0,0x8000,0,0,0],3,4),4);
  assert.equal(state(q.a).regs[0],(group<16?0x7f00:0)|group);
 }
});


test('a keyboard compound declines changed vectors, ROM, mapped and self-code frames before INT',async()=>{
 for(const scenario of['vector','rom','wrap','self','mapped','deadline','bitmap-alias','vector-alias']){
  const code=[0xb4,4,0xcd,0x18,0xeb,0xfa];
  const a=await boot(code),b=await boot(code);
  for(const c of[a,b])setState(c,[0x0403,0,0,0,0x8000,0,0,0]);
  const p=await translate(a,b,1,3);
  let sp=0x8000,cap=4,clocks=80000;
  for(const c of[a,b]){
   const ram=c.block_ram_ptr()>>>0,mem=new Uint8Array(c.memory.buffer);
   if(scenario==='vector')mem[ram+0x60]=0x61;
   if(scenario==='rom')mem[ram+0xf1062]=0x90;
   if(scenario==='wrap')sp=4;
   if(scenario==='self'){
    // Load an ordinary SS pointing at the code segment.
    mem.set([0xb8,0xc0,0x1f,0x8e,0xd0],ram+0x1fd00);const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(2);sp=10;
   }
   if(scenario==='mapped'){
    mem.set([0xb8,0,0xa0,0x8e,0xd0],ram+0x1fd00);const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(2);
   }
   if(scenario==='deadline')clocks=1;
   if(scenario==='bitmap-alias')sp=0x52f; // FLAGS low byte lands on queried group 3.
   if(scenario==='vector-alias')sp=0x66; // Vector is loaded before the pushes overwrite it.
  }
  const done=execute(p,[0x0403,0,0,0,sp,0,0,0],0xcd7,cap,clocks);
  assert.equal(done,['bitmap-alias','vector-alias'].includes(scenario)?4:1,scenario);
  if(scenario==='bitmap-alias')assert.equal(state(a).regs[0],0xd703);
 }
});


test('query admission checks current AH and preserves upper flag clearing across later operations',async()=>{
 const a=await boot([0xb0,3,0xcd,0x18,0x40,0xeb,0xf9]),b=await boot([0xb0,3,0xcd,0x18,0x40,0xeb,0xf9]);
 for(const c of[a,b])setState(c,[0x0403,0,0,0,0x8000,0,0,0]);
 const p=await translate(a,b,1,3);
 assert.equal(execute(p,[0x0103,0,0,0,0x8000,0,0,0],3,10),1);
 // POPFD sets AC using the real instruction path; a committed query
 // must clear it even if the last operation in the trace is an ordinary INC.
 for(const c of[a,b]){
  const ram=c.block_ram_ptr()>>>0;new Uint8Array(c.memory.buffer).set([0x66,0x9d],ram+0x1fd00);
  new DataView(c.memory.buffer).setUint32(ram+0x9000,0x50003,true);setState(c,[0,0,0,0,0x9000,0,0,0],3,0x100);c.step(1);assert.equal(c.block_debug(7),4);
 }
 assert.equal(execute(p,[0x0403,0,0,0,0x8000,0,0,0],3,5),5);assert.equal(a.block_debug(7),0);
});


test('a query uses an unaligned relocated IVT and declines a shortened live IDT limit',async()=>{
 const a=await boot([0xb4,4,0xcd,0x18,0xeb,0xfa]),b=await boot([0xb4,4,0xcd,0x18,0xeb,0xfa]);
 function loadIdt(c,limit){
  const ram=c.block_ram_ptr()>>>0,mem=new Uint8Array(c.memory.buffer),v=new DataView(c.memory.buffer);
  mem.set([0xb8,0,0,0x8e,0xd8,0x0f,0x01,0x1e,0,0x90],ram+0x1fd00);
  v.setUint16(ram+0x9000,limit,true);v.setUint32(ram+0x9002,0x6001,true);mem.set([0x60,0x10,0,0xf0],ram+0x6061);
  setState(c,[0x0403,0,0,0,0x8000,0,0,0],3,0x100);c.step(3);setState(c,[0x0403,0,0,0,0x8000,0,0,0]);
 }
 for(const c of[a,b])loadIdt(c,0x3ff);
 const p=await translate(a,b,1,3),before=a.metric(26);
 assert.equal(execute(p,[0x0403,0,0,0,0x8000,0,0,0],3,4),4);assert.equal(a.metric(26)-before,127);
 for(const c of[a,b])loadIdt(c,0x60);
 assert.equal(execute(p,[0x0403,0,0,0,0x8000,0,0,0],3,4),1);
});

test('a learned taken successor still exits on a later fallthrough without executing the predicted path',async()=>{
 const code=[0x3d,0,0,0x75,3,0x43,0xeb,0xf8,0x42,0xeb,0xf5];
 const a=await boot(code),b=await boot(code),j=new BlockJit(a);
 for(const c of[a,b])setState(c,[1,0,0,10,0x8000,0,0,0]);
 for(let n=0;n<80;n++){
  const done=j.step(8000,1000);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();
 }
 assert(a.jit_metric(2)>0,'the hot taken exit was never promoted');
 const dx=state(a).regs[2],bx=state(a).regs[3];
 for(const c of[a,b]){const s=state(c);s.regs[0]=0;setState(c,s.regs,3);}
 const done=j.step(8000,30);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));equal(a,b);
 assert.equal(state(a).regs[2],dx);assert(state(a).regs[3]>bx);
});

test('a preceding trace store can change the IVT and the query guard rereads it before INT',async()=>{
 const code=[0xb4,4,0xc6,0x06,0x60,0,0x61,0xcd,0x18,0xeb,0xf5];
 const a=await boot(code),b=await boot(code);
 for(const c of[a,b])setState(c,[0x0403,0,0,0,0x8000,0,0,0]);
 const p=await translate(a,b,1,3);
 assert.equal(execute(p,[0x0403,0,0,0,0x8000,0,0,0],3,10),2);
 assert.equal(state(a).ip,8);assert.equal(state(a).regs[4],0x8000);assert.equal(a.peek(0x60),0x61);
});


test('ordinary direct plans preserve the legacy 80-byte completion context boundary',async()=>{
 const memory=new WebAssembly.Memory({initial:1}),v=new DataView(memory.buffer),scratch=512;
 v.setUint16(300,2,true);v.setUint32(304,0x100,true);
 v.setUint32(scratch+4,100,true);v.setUint32(scratch+8,1,true);
 new Uint8Array(memory.buffer,scratch+80,24).fill(0xa5);
 const packet=Uint32Array.from([1,1,0x100,1,0x40,0]);
 const meta=Uint32Array.from([1,0,1,1,0,8,8,0,0,0,0,0,1,4]);
 let finished=0;
 const bytes=compileRamBlock(packet,meta),run=new WebAssembly.Instance(await WebAssembly.compile(bytes),{core:{memory,finish:()=>finished++}}).exports.run;
 run(256,300,304,scratch);
 assert.equal(finished,1);assert.equal(v.getUint16(256,true),1);assert.equal(v.getUint32(304,true),0x101);assert.equal(v.getUint32(scratch+12,true),1);
 assert.deepEqual(Array.from(new Uint8Array(memory.buffer,scratch+80,24)),Array(24).fill(0xa5));
});
