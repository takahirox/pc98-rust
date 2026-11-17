// Link validated RAM traces only within the core's current device-event budget.
const uleb=value=>{const out=[];do{const b=value&127;value>>>=7;out.push(b|(value?128:0));}while(value);return out;};
const sleb=value=>{const out=[];value|=0;for(;;){const b=value&127;value>>=7;const done=value===0&&!(b&64)||value===-1&&(b&64);out.push(b|(done?0:128));if(done)return out;}};
const text=value=>{const bytes=new TextEncoder().encode(value);return [...uleb(bytes.length),...bytes];};
const section=(id,bytes)=>[id,...uleb(bytes.length),...bytes];
const get=n=>[0x20,n],set=(n,value)=>[...value,0x21,n],c=n=>[0x41,...sleb(n)];
const bin=(op,a,b)=>[...a,...b,op];
const load=(address,offset=0)=>[...address,0x28,2,...uleb(offset)];

export function createBlockLinker(core,table){
  // Parameters are the four CPU layout pointers and compilation permission.
  // Locals: packed dispatch result (i64), ID, context, previous units, CS, IP,
  // link address. A miss or guard failure retires the accumulated run in Rust.
  const ctx=i=>load(get(7),i*4),reject=value=>[...value,0x0d,1];
  const call=[...get(0),...get(1),...get(2),...get(3),...bin(0x71,get(6),c(4095)),0x11,2,0];
  const body=[2,1,0x7e,6,0x7f,0x03,0x40,
    ...get(4),0x10,0,0x22,5,0x50,0x04,0x40,...c(0),0x0f,0x0b,
    ...set(6,[...get(5),0xa7]),...get(5),0x42,32,0x88,0xa7,0x04,0x40,...get(6),0x0f,0x0b,
    ...set(7,bin(0x6a,get(3),c(core.block_context_offset()))),
    ...bin(0x71,ctx(27),c(1)),0x04,0x40,
      0x02,0x40,0x03,0x40,
        ...set(8,ctx(3)),...call,
        ...reject(bin(0x46,ctx(3),get(8))),
        ...reject([...bin(0x71,ctx(27),c(2)),0x45]),
        ...reject(bin(0x4f,ctx(3),ctx(2))),
        ...reject(bin(0x4f,ctx(4),ctx(1))),
        ...set(9,ctx(13)),...set(10,load(get(2))),
        ...set(11,bin(0x6a,c(core.block_links_ptr()),bin(0x6c,c(12),bin(0x71,
          bin(0x73,bin(0x73,bin(0x73,get(10),bin(0x76,get(10),c(7))),bin(0x76,get(10),c(29))),bin(0x74,get(9),c(3))),c(16383))))),
        ...reject(bin(0x47,load(get(11)),get(10))),
        ...reject(bin(0x47,load(get(11),4),get(9))),
        ...set(6,load(get(11),8)),...reject([...get(6),0x45]),
        0x0c,0,0x0b,0x0b,0x10,1,
    0x05,...call,0x0b,
    0x0c,0,0x0b,...c(0),0x0b];
  const types=[4,0x60,1,0x7f,1,0x7e,0x60,0,0,0x60,4,0x7f,0x7f,0x7f,0x7f,0,0x60,5,0x7f,0x7f,0x7f,0x7f,0x7f,1,0x7f];
  const imports=[4,...text('core'),...text('dispatch'),0,0,...text('core'),...text('finish'),0,1,
    ...text('core'),...text('table'),1,0x70,0,...uleb(4096),...text('core'),...text('memory'),2,0,0];
  const bytes=new Uint8Array([0,97,115,109,1,0,0,0,...section(1,types),...section(2,imports),
    ...section(3,[2,3,1]),...section(7,[2,...text('run'),0,2,...text('finish'),0,3]),
    ...section(10,[2,...uleb(body.length),...body,2,0,0x0b])]);
  return new WebAssembly.Instance(new WebAssembly.Module(bytes),{core:{memory:core.memory,table,dispatch:core.jit_dispatch_entry,finish:core.block_finish}}).exports;
}
