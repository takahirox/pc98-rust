// Opt-in actual-Wasm conformance for the non-published block experiment.
import {test} from 'node:test';import assert from 'node:assert/strict';import {readFile} from 'node:fs/promises';
import {compileRamBlock} from '../tools/lib/block-compiler.mjs';
import {BlockJit} from '../tools/lib/block-jit.mjs';
if(!process.env.PC98_BLOCK_WASM)throw Error('Set PC98_BLOCK_WASM to the feature-enabled release Wasm outside web/');
const module=await WebAssembly.compile(await readFile(process.env.PC98_BLOCK_WASM));
const layout=c=>Array.from(new Uint32Array(c.memory.buffer,c.jit_layout()>>>0,4));
function state(c){const[r,f,p]=layout(c),v=new DataView(c.memory.buffer);return {regs:Array.from(new Uint16Array(c.memory.buffer,r,8)),flags:v.getUint16(f,true),ip:v.getUint32(p,true),cs:c.metric(17),metrics:[0,1,2,5,6,7,9,10,14,15,19,20,23,26,27,28,30].map(i=>c.metric(i)),debug:Array.from({length:9},(_,i)=>c.block_debug(i)),fault:c.metric(4)};}
function setState(c,regs,flags=2,ip=1){const[r,f,p]=layout(c),v=new DataView(c.memory.buffer);new Uint16Array(c.memory.buffer,r,8).set(regs);v.setUint16(f,flags,true);v.setUint32(p,ip,true);}
function equal(a,b){assert.deepEqual(state(a),state(b));const frame=c=>Buffer.from(c.memory.buffer,c.frame()>>>0,1024000);assert(frame(a).equals(frame(b)),'rendered frame differs');const ram=c=>Buffer.from(c.memory.buffer,c.block_ram_ptr()>>>0,0xa0000);assert(ram(a).equals(ram(b)),'conventional RAM differs');const pcm=c=>Buffer.from(c.memory.buffer,c.audio_ptr()>>>0,c.audio_len()*4);assert(pcm(a).equals(pcm(b)),'PCM bits differ');}
async function boot(code,patch=[]){const c=new WebAssembly.Instance(module,{}).exports;c.set_profile(1);c.set_boot_source(0);const disk=new Uint8Array(1261568);disk.set([0xfa,...code]);disk.set(patch,0x100);const p=c.input_buffer(disk.length)>>>0;new Uint8Array(c.memory.buffer,p,disk.length).set(disk);assert.equal(c.insert(0,0),1);c.reset();for(let n=0;c.metric(17)!==0x1fc0||c.metric(18)!==1;n++){assert(n<2000);assert.equal(c.metric(4),0);c.step(1);}return c;}
async function pair(code,patch=[],initial=null){const a=await boot(code,patch),b=await boot(code,patch);if(initial)for(const c of[a,b])setState(c,initial);let id=0;for(let n=0;!id;n++){assert(n<64);for(const c of[a,b]){const s=state(c);setState(c,s.regs,s.flags,1);}a.jit_devices_begin();a.block_begin_slice(80000,1);id=a.jit_dispatch(1);a.jit_devices_end();b.step(a.jit_done());}const packet=new Uint32Array(a.memory.buffer,a.jit_packet()>>>0,a.jit_packet_len()).slice(),meta=new Uint32Array(a.memory.buffer,a.block_packet()>>>0,a.block_packet_len()).slice();assert.equal(meta[0],1,'probe is not a direct RAM plan');const bytes=compileRamBlock(packet,meta);assert(WebAssembly.validate(bytes));const run=new WebAssembly.Instance(await WebAssembly.compile(bytes),{core:{memory:a.memory,finish:a.block_finish}}).exports.run;assert.equal(a.jit_register(id,1),1);equal(a,b);return {a,b,id,run,packet,meta};}
function execute(p,regs,flags=2,cap=100,clocks=80000){for(const c of[p.a,p.b])setState(c,regs,flags,1);p.a.jit_devices_begin();p.a.block_begin_slice(clocks,cap);if(p.a.block_start(p.id))p.run(...layout(p.a));else p.a.jit_fallback();p.a.jit_devices_end();const done=p.a.jit_done();p.b.step(done);equal(p.a,p.b);return done;}

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

test('mapped operands and a 16-bit boundary decline before bus effects',async()=>{
 for(const bx of [0xffff,0x8000]){
  const p=await pair([0x89,0x07,0x40,0xeb,0xfb]);
  if(bx===0x8000){for(const c of[p.a,p.b]){const ptr=c.block_ram_ptr()>>>0;new Uint8Array(c.memory.buffer,ptr+0x1fd00,5).set([0xb8,0,0xa8,0x8e,0xd8]);const s=state(c);setState(c,s.regs,s.flags,0x100);c.step(2);}}
  execute(p,[0x9876,0,0,bx,0,0,0,0],2,1);assert(p.a.block_metric(2)>0);
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
 const code=[0x8b,0x07,0x40,0x89,0x07,...Array(13).fill(0x40),0xd1,0xe0,0xeb,0xea]; // shift remains guarded
 const a=await boot(code),b=await boot(code),j=new BlockJit(a);
 for(const c of[a,b])setState(c,[0,0,0,0x8000,0,0,0,0]);
 for(let n=0;n<40;n++){const done=j.step(8000,1000);b.step(done);while(j.pending)await new Promise(r=>setTimeout(r,0));assert(!j.failed,j.compileError);equal(a,b);a.audio_clear();b.audio_clear();}
 assert(j.stats().blockInstructions>0);assert(a.jit_metric(1)>j.stats().blockInstructions);
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
