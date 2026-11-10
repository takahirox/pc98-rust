// Original x86 register-block -> Wasm compiler. No eval, external service or ISA library.
const uleb=value=>{const out=[];do{let b=value&127;value>>>=7;out.push(b|(value?128:0));}while(value);return out;};
const sleb=value=>{const out=[];value|=0;for(;;){const b=value&127;value>>=7;const done=value===0&&!(b&64)||value===-1&&(b&64);out.push(b|(done?0:128));if(done)return out;}};
const text=value=>{const bytes=new TextEncoder().encode(value);return [...uleb(bytes.length),...bytes];};
const section=(id,bytes)=>[id,...uleb(bytes.length),...bytes];
const get=n=>[0x20,n], set=(n,value)=>[...value,0x21,n], c=value=>[0x41,...sleb(value)];
const bin=(op,a,b)=>[...a,...b,op], and=(a,b)=>bin(0x71,a,b), or=(a,b)=>bin(0x72,a,b), xor=(a,b)=>bin(0x73,a,b);
const add=(a,b)=>bin(0x6a,a,b), sub=(a,b)=>bin(0x6b,a,b), shl=(a,n)=>bin(0x74,a,c(n));
const nz=a=>[...a,0x45,0x45], not=a=>[...a,0x45];
const load=(p,offset,width)=>[...get(p),width===1?0x2d:0x2f,width===1?0:1,...uleb(offset)];
const store=(p,offset,width,value)=>[...get(p),...value,width===1?0x3a:0x3b,width===1?0:1,...uleb(offset)];
const reg=(r,wide)=>load(0,wide?r*2:(r&3)*2+(r>>2),wide?2:1);
const put=(r,wide,value)=>store(0,wide?r*2:(r&3)*2+(r>>2),wide?2:1,value);
const flag=mask=>and(get(6),c(mask));
function condition(code){
  let value;
  switch(code&14){
    case 0:value=flag(0x800);break;
    case 2:value=flag(1);break;
    case 4:value=flag(0x40);break;
    case 6:value=flag(0x41);break;
    case 8:value=flag(0x80);break;
    case 10:value=flag(4);break;
    case 12:value=xor(nz(flag(0x80)),nz(flag(0x800)));break;
    case 14:value=or(nz(flag(0x40)),xor(nz(flag(0x80)),nz(flag(0x800))));break;
  }
  return code&1?not(value):nz(value);
}
function arithmetic(kind,wide,dst,a,b,{write=true,increment=false}={}){
  const mask=wide?65535:255, sign=wide?32768:128;
  const carry=kind===2||kind===3?flag(1):c(0);
  let value;
  switch(kind){
    case 0:case 2:value=add(add(get(3),get(4)),get(7));break;
    case 1:value=or(get(3),get(4));break;
    case 3:case 5:case 7:value=sub(sub(get(3),get(4)),get(7));break;
    case 4:value=and(get(3),get(4));break;
    case 6:value=xor(get(3),get(4));break;
  }
  const out=[...set(3,a),...set(4,and(b,c(mask))),...set(7,carry),...set(5,and(value,c(mask)))];
  if(write)out.push(...put(dst,wide,get(5)));
  const parts=[and(get(6),c(~(increment?0x8d4:0x8d5))),
    shl(not(get(5)),6),shl(nz(and(get(5),c(sign))),7),
    shl(not(and([...and(get(5),c(255)),0x69],c(1))),2)];
  if([0,2,3,5,7].includes(kind)){
    const subtract=[3,5,7].includes(kind);
    if(!increment)parts.push(subtract?bin(0x49,get(3),add(get(4),get(7))):bin(0x4b,add(add(get(3),get(4)),get(7)),c(mask)));
    parts.push(and(xor(xor(get(3),get(4)),get(5)),c(16)));
    const signs=xor(get(3),get(4));
    parts.push(shl(nz(and(and(subtract?signs:xor(signs,c(-1)),xor(get(3),get(5))),c(sign))),11));
  }
  out.push(...store(1,0,2,parts.reduce(or)));
  return out;
}

/** Compile only Rust-preflighted unprefixed real-mode instructions. */
export function compileBlock(packet){
  const count=packet[1];
  if(!Number.isInteger(count)||count<2||count>16||packet.length!==2+count*3)throw Error('Invalid JIT plan');
  const body=[1,6,0x7f]; // Parameters: register/flags/IP addresses. Six i32 locals.
  for(let index=0;index<count;index++){
    const ip=packet[2+index*3],length=packet[3+index*3],encoded=packet[4+index*3];
    const bytes=[encoded&255,(encoded>>>8)&255,(encoded>>>16)&255,encoded>>>24];
    const [op,m]=bytes, wide=!!(op&1), rm=m&7, r=(m>>3)&7;
    const imm=(at,w)=>w?bytes[at]|bytes[at+1]<<8:bytes[at];
    body.push(...set(6,load(1,0,2)),...set(8,c(0)));
    if(op<=0x3d&&(op&7)<=5){
      const kind=op>>3, direct=(op&7)>=4, dst=direct?0:op&2?r:rm;
      body.push(...arithmetic(kind,wide,dst,reg(dst,wide),direct?c(imm(1,wide)):reg(op&2?rm:r,wide),{write:kind!==7}));
    }else if(op>=0x40&&op<=0x4f){
      body.push(...arithmetic(op&8?5:0,true,op&7,reg(op&7,true),c(1),{increment:true}));
    }else if(op>=0x70&&op<=0x7f){
      body.push(...set(8,condition(op&15)));
    }else if([0x80,0x81,0x83].includes(op)){
      const value=op===0x83?(bytes[2]<<24)>>24:imm(2,wide);
      body.push(...arithmetic(r,wide,rm,reg(rm,wide),c(value),{write:r!==7}));
    }else if(op===0x84||op===0x85){
      body.push(...arithmetic(4,wide,rm,reg(rm,wide),reg(r,wide),{write:false}));
    }else if(op===0x86||op===0x87){
      body.push(...set(3,reg(rm,wide)),...put(rm,wide,reg(r,wide)),...put(r,wide,get(3)));
    }else if(op>=0x88&&op<=0x8b){
      body.push(...put(op&2?r:rm,wide,reg(op&2?rm:r,wide)));
    }else if(op>=0x90&&op<=0x97){
      body.push(...set(3,reg(0,true)),...put(0,true,reg(op&7,true)),...put(op&7,true,get(3)));
    }else if(op===0x9e){
      body.push(...store(1,0,2,or(or(and(get(6),c(~0xd5)),and(reg(4,false),c(0xd5))),c(2))));
    }else if(op===0x9f){
      body.push(...put(4,false,get(6)));
    }else if(op===0xa8||op===0xa9){
      body.push(...arithmetic(4,wide,0,reg(0,wide),c(imm(1,wide)),{write:false}));
    }else if(op>=0xb0&&op<=0xbf){
      const word=op>=0xb8;body.push(...put(op&7,word,c(imm(1,word))));
    }else if(op===0xc6||op===0xc7){
      body.push(...put(rm,wide,c(imm(2,wide))));
    }else if(op>=0xe0&&op<=0xe3){
      if(op!==0xe3)body.push(...put(1,true,sub(reg(1,true),c(1))));
      let take=op===0xe3?not(reg(1,true)):nz(reg(1,true));
      if(op===0xe0||op===0xe1)take=and(take,op===0xe0?not(flag(0x40)):nz(flag(0x40)));
      body.push(...set(8,take));
    }else if(op===0xe9||op===0xeb){
      body.push(...set(8,c(1)));
    }else if([0xf5,0xf8,0xf9,0xfc,0xfd].includes(op)){
      const bits=op>=0xfc?1024:1;
      body.push(...store(1,0,2,op===0xf5?xor(get(6),c(1)):op===0xf8||op===0xfc?and(get(6),c(~bits)):or(get(6),c(bits))));
    }else throw Error('Unsupported JIT opcode');
    const branch=op>=0x70&&op<=0x7f||op>=0xe0&&op<=0xe3||op===0xe9||op===0xeb;
    if(branch){
      const displacement=op===0xe9?((imm(1,true)<<16)>>16):((bytes[1]<<24)>>24);
      body.push(...get(8),0x04,0x40,...get(2),...c((ip+length+displacement)&65535),0x36,2,0,0x0b);
    }
    // One host callback per instruction: commit/tick, then guard the next fetch.
    body.push(...c(index+1),...(op===0xe9||op===0xeb?c(0):get(8)),0x10,0,0x45,0x04,0x40,0x0f,0x0b);
  }
  body.push(0x0b);
  const types=[2,0x60,2,0x7f,0x7f,1,0x7f,0x60,3,0x7f,0x7f,0x7f,0];
  const imports=[2,...text('core'),...text('next'),0,0,...text('core'),...text('memory'),2,0,0];
  return new Uint8Array([0,97,115,109,1,0,0,0,
    ...section(1,types),...section(2,imports),...section(3,[1,1]),
    ...section(7,[1,...text('run'),0,1]),...section(10,[1,...uleb(body.length),...body])]);
}

export class WasmJit {
  constructor(core,{compile=bytes=>WebAssembly.compile(bytes)}={}){
    this.core=core;this.compile=compile;this.cache=new Map();this.pending=null;this.layout=null;
    this.failed=false;this.compiled=0;this.compilationMs=0;this.bytes=0;this.failures=0;
  }
  clear(){if(this.pending)this.pending.cancelled=true;this.layout=null;this.cache.clear();this.compiled=0;this.compilationMs=0;this.bytes=0;this.core.jit_clear();}
  stats(){return {mode:this.failed?'interpreter':'jit',compiled:this.compiled,cached:this.cache.size,
    compilationMs:this.compilationMs,generatedBytes:this.bytes,failures:this.failures,
    offers:this.core.jit_metric(0),instructions:this.core.jit_metric(1),invalidations:this.core.jit_metric(2)};}
  offer(id){
    const core=this.core,ptr=core.jit_packet()>>>0,length=core.jit_packet_len();
    const packet=new Uint32Array(core.memory.buffer,ptr,length).slice();
    if(packet[0]!==id)throw Error('JIT plan identity mismatch');
    const token={cancelled:false};this.pending=token;
    const started=performance.now();
    Promise.resolve().then(()=>{if(token.cancelled)return null;const bytes=compileBlock(packet);this.bytes+=bytes.length;return this.compile(bytes);})
      .then(module=>{
        if(this.pending!==token||token.cancelled)return;
        const instance=new WebAssembly.Instance(module,{core:{memory:core.memory,next:core.jit_next}});
        if(!core.jit_register(id,1))return; // Reset/eviction may invalidate an offer.
        if(this.cache.size>=128){const old=this.cache.keys().next().value;this.cache.delete(old);core.jit_register(old,0);}
        this.cache.set(id,instance.exports.run);this.compiled++;
      }).catch(()=>{
        if(this.pending===token&&!token.cancelled){core.jit_register(id,0);this.failed=true;this.failures++;}
      }).finally(()=>{
        if(this.pending===token){if(!token.cancelled)this.compilationMs+=performance.now()-started;this.pending=null;}
      });
  }
  step(clocks,cap){
    const core=this.core;
    if(this.failed||core.metric(31)!==66000000)return core.step_scheduler(clocks,cap);
    core.jit_begin_slice(clocks,cap);
    for(;;){
      const id=core.jit_dispatch(this.pending?0:1);
      if(!id)break;
      const run=this.cache.get(id);
      if(!run){this.offer(id);continue;}
      if(!core.jit_start(id)){core.jit_fallback();continue;}
      if(!this.layout){
        const ptr=core.jit_layout()>>>0;
        // Copy addresses, not a view that can detach on memory.grow. CPU fields
        // stay in the owning instance; reset/control barriers refresh this tuple.
        this.layout=Array.from(new Uint32Array(core.memory.buffer,ptr,3));
      }
      run(this.layout[0],this.layout[1],this.layout[2]);
    }
    return core.jit_done();
  }
}
