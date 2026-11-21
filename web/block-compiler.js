// Event-bounded conventional-RAM trace emitter for the browser CPU.
// Original x86 block -> Wasm compiler. No eval, external service or ISA library.
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
const storeFlags=value=>[...get(1),...value,0x22,7,0x3b,1,0];
const reg=(r,wide)=>load(0,wide?r*2:(r&3)*2+(r>>2),wide?2:1);
const put=(r,wide,value)=>store(0,wide?r*2:(r&3)*2+(r>>2),wide?2:1,value);
const flag=mask=>and(get(7),c(mask));
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
function arithmetic(kind,wide,dst,a,b,{write=true,increment=false,writeFlags=storeFlags,putRegister=put,flagMask=0x8d5}={}){
  const mask=wide?65535:255, sign=wide?32768:128;
  const changed=flagMask&(increment?0x8d4:0x8d5);
  if(!write&&!changed)return [];
  const carry=kind===2||kind===3?flag(1):c(0);
  let value;
  switch(kind){
    case 0:case 2:value=add(add(get(4),get(5)),get(8));break;
    case 1:value=or(get(4),get(5));break;
    case 3:case 5:case 7:value=sub(sub(get(4),get(5)),get(8));break;
    case 4:value=and(get(4),get(5));break;
    case 6:value=xor(get(4),get(5));break;
  }
  const out=[...set(4,a),...set(5,and(b,c(mask))),...set(8,carry),...set(6,and(value,c(mask)))];
  if(write)out.push(...putRegister(dst,wide,get(6)));
  if(!changed)return out;
  const parts=[and(get(7),c(~changed))];
  if(changed&0x40)parts.push(shl(not(get(6)),6));
  if(changed&0x80)parts.push(shl(nz(and(get(6),c(sign))),7));
  if(changed&4)parts.push(shl(not(and([...and(get(6),c(255)),0x69],c(1))),2));
  if([0,2,3,5,7].includes(kind)){
    const subtract=[3,5,7].includes(kind);
    if(changed&1)parts.push(subtract?bin(0x49,get(4),add(get(5),get(8))):bin(0x4b,add(add(get(4),get(5)),get(8)),c(mask)));
    if(changed&16)parts.push(and(xor(xor(get(4),get(5)),get(6)),c(16)));
    const signs=xor(get(4),get(5));
    if(changed&0x800)parts.push(shl(nz(and(and(subtract?signs:xor(signs,c(-1)),xor(get(4),get(6))),c(sign))),11));
  }
  out.push(...writeFlags(parts.reduce(or)));
  return out;
}

function singleShift(kind,wide,dst,writeFlags=storeFlags,readRegister=reg,putRegister=put,flagMask=0x8d5){
  const mask=wide?65535:255,sign=wide?32768:128;
  const out=[...set(4,readRegister(dst,wide)),...set(8,nz(and(get(4),c([0,2,4,6].includes(kind)?sign:1))))];
  let value;
  switch(kind){
    case 0:value=or(shl(get(4),1),get(8));break;
    case 1:value=or(bin(0x76,get(4),c(1)),shl(get(8),wide?15:7));break;
    case 2:value=or(shl(get(4),1),flag(1));break;
    case 3:value=or(bin(0x76,get(4),c(1)),shl(flag(1),wide?15:7));break;
    case 4:case 6:value=shl(get(4),1);break;
    case 5:value=bin(0x76,get(4),c(1));break;
    case 7:value=or(bin(0x76,get(4),c(1)),and(get(4),c(sign)));break;
  }
  out.push(...set(6,and(value,c(mask))),...putRegister(dst,wide,get(6)));
  const changed=flagMask&(0x801|(kind>=4?0xc4:0));
  if(!changed)return out;
  const overflow=[0,2,4,6].includes(kind)?xor(nz(and(get(6),c(sign))),get(8)):
    [1,3].includes(kind)?xor(nz(and(get(6),c(sign))),nz(and(get(6),c(sign>>1)))):
    kind===5?nz(and(get(4),c(sign))):c(0);
  // AF is preserved, including the existing deterministic policy for shifts.
  const parts=[and(get(7),c(~changed))];
  if(changed&1)parts.push(get(8));
  if(changed&0x800)parts.push(shl(overflow,11));
  if(changed&0x40)parts.push(shl(not(get(6)),6));
  if(changed&0x80)parts.push(shl(nz(and(get(6),c(sign))),7));
  if(changed&4)parts.push(shl(not(and([...and(get(6),c(255)),0x69],c(1))),2));
  out.push(...writeFlags(parts.reduce(or)));
  return out;
}

function constantShift(kind,wide,dst,count,writeFlags,readRegister,putRegister,flagMask){
  if(count===1)return singleShift(kind,wide,dst,writeFlags,readRegister,putRegister,flagMask);
  const read=readRegister(dst,wide);
  const write=value=>putRegister(dst,wide,value);
  if(!count)return write(read);
  const bits=wide?16:8,mask=wide?65535:255;
  const right=(value,n)=>bin(0x76,value,c(n));
  let value,carry;
  if(kind===0||kind===1){
    const n=count%bits;
    value=n?(kind===0?or(shl(get(4),n),right(get(4),bits-n)):or(right(get(4),n),shl(get(4),bits-n))):get(4);
    carry=kind===0?and(value,c(1)):and(right(value,bits-1),c(1));
  }else if(kind===4||kind===6){
    value=count>=bits?c(0):shl(get(4),count);
    carry=count>bits?c(0):and(right(get(4),bits-count),c(1));
  }else if(kind===5){
    value=count>=bits?c(0):right(get(4),count);
    carry=count>bits?c(0):and(right(get(4),count-1),c(1));
  }else if(kind===7){
    value=bin(0x75,[...get(4),wide?0xc1:0xc0],c(count));
    carry=and(right(get(4),Math.min(count-1,bits-1)),c(1));
  }else throw Error('Unsupported JIT immediate carry rotation');
  const out=[...set(4,read),...set(6,and(value,c(mask))),...write(get(6))];
  // The interpreter preserves OF for counts other than one and AF always.
  const changed=flagMask&(1|(kind>=4?0xc4:0));
  if(!changed)return out;
  const parts=[and(get(7),c(~changed))];
  if(changed&1)parts.push(carry);
  if(changed&0x40)parts.push(shl(not(get(6)),6));
  if(changed&0x80)parts.push(shl(nz(and(get(6),c(wide?32768:128))),7));
  if(changed&4)parts.push(shl(not(and([...and(get(6),c(255)),0x69],c(1))),2));
  out.push(...writeFlags(parts.reduce(or)));
  return out;
}

function writtenRegisters(op,m){
  const wide=!!(op&1),rm=m&7,r=(m>>3)&7;
  const target=n=>1<<(wide?n:n&3);
  if(op<=0x3d&&(op&7)<=5){
    if(op>>3===7)return 0;
    if((op&7)>=4)return 1;
    return op&2?target(r):m>=0xc0?target(rm):0;
  }
  if(op>=0x40&&op<=0x4f)return 1<<(op&7);
  if(op>=0x50&&op<=0x57||op===0xc3||op===0xe8)return 1<<4;
  if(op>=0x58&&op<=0x5f)return (1<<4)|(1<<(op&7));
  if(op>=0x70&&op<=0x7f||[0x84,0x85,0x9e,0xa8,0xa9,0xe3,0xe9,0xeb,0xf5,0xf8,0xf9,0xfc,0xfd].includes(op))return 0;
  if([0x80,0x81,0x83].includes(op))return r===7||m<0xc0?0:target(rm);
  if(op===0x86||op===0x87)return target(r)|(m>=0xc0?target(rm):0);
  if(op===0x88||op===0x89||op===0xc6||op===0xc7)return m>=0xc0?target(rm):0;
  if(op===0x8a||op===0x8b||op===0x8d)return target(r);
  if(op===0x8c)return 1<<rm;
  if(op===0xc4||op===0xc5)return 1<<r;
  if(op>=0x90&&op<=0x97)return 1|(1<<(op&7));
  if(op===0x9f||op===0xa0||op===0xa1)return 1;
  if(op===0xa2||op===0xa3)return 0;
  if(op>=0xb0&&op<=0xbf)return 1<<(op>=0xb8?op&7:op&3);
  if([0xc0,0xc1,0xd0,0xd1,0xfe,0xff].includes(op))return m<0xc0?0:target(rm);
  if(op>=0xe0&&op<=0xe2)return 1<<1;
  if(op===0xf6||op===0xf7)return r===0||m<0xc0?0:target(rm);
  throw Error('Missing JIT register-write policy');
}

export function compileRamBlock(packet,meta){
  const count=packet[1];
  const linked=!!(meta[0]&4),header=linked?5:2,stride=linked?14:12;
  if(count<1||count>(linked?32:16)||packet.length!==2+count*4||meta.length!==header+count*stride||![1,3,5,7].includes(meta[0]))throw Error('Invalid RAM block plan');
  const bios=!!(meta[0]&2);
  const highCode=linked && meta[2]+packet[2]>=0x100000;
  // Four pointer parameters. Locals 4..52 contain ALU temporaries, cached GPRs,
  // context addresses, timing and the last committed instruction metadata.
  // Extended packets carry raw code guards and a run-wide final plan ID.
  const body=[1,linked?49:bios?48:43,0x7f],dirty=new Set(),used=new Set();
  const readReg=(r,wide)=>{used.add(wide?r:r&3);return wide?get(15+r):and(r&4?bin(0x76,get(15+(r&3)),c(8)):get(15+(r&3)),c(255));};
  const writeReg=(r,wide,value)=>{const word=wide?r:r&3;dirty.add(word);used.add(word);return set(15+word,wide?and(value,c(65535)):or(and(get(15+word),c(r&4?255:65280)),r&4?shl(and(value,c(255)),8):and(value,c(255))));};
  const context=i=>[...get(14),0x28,2,...uleb(i*4)];
  const save=(i,value)=>[...get(14),...value,0x36,2,...uleb(i*4)];
  const selector=i=>load(14,116+2*i,2);
  const exitIf=value=>body.push(...value,0x0d,1);
  body.push(...set(14,add(get(3),c(meta[1]))),...set(7,load(1,0,2)),...set(33,[...get(2),0x28,2,0]),
    ...(linked?[...set(27,context(4)),...set(28,context(3)),...set(29,context(6)),...set(30,context(7)),...set(31,context(8)),...set(32,context(9)),...set(45,context(18)),...set(46,context(19)),...set(47,context(24)),...set(48,context(25)),...set(52,context(26))]:bios?set(47,c(1)):[]),
    ...set(25,context(5)),...set(34,context(0)),...set(35,context(1)),...set(36,context(2)),...set(37,linked?c(meta[3]):context(10)),...set(38,linked?c(meta[4]):context(11)));
  const reloadAt=body.length;
  const segments=new Set(Array.from({length:count},(_,i)=>meta[header+i*stride+7]&3?meta[header+i*stride+2]:-1).filter(i=>i>=0));
  if(Array.from({length:count},(_,i)=>packet[4+i*4]&255).includes(0xc3))segments.add(1);
  for(const seg of segments)body.push(...set(39+seg,context(12+seg)));
  body.push(0x02,0x40);
  if(linked){
    // Linked entries bypass the Rust decoder, so revalidate the complete code
    // before effects. Adjacent bytes share loads; no load crosses a code span.
    const reject=value=>body.push(...value,0x0d,0);
    reject(bin(0x47,get(33),c(packet[2])));
    reject(bin(0x47,context(13),c(meta[2])));
    const spans=[];
    for(let i=0;i<count;i++){
      const ip=packet[2+i*4],length=packet[3+i*4],at=header+i*stride;
      let span=spans.at(-1);
      if(!span||span.ip+span.bytes.length!==ip){span={ip,bytes:[]};spans.push(span);}
      for(let j=0;j<length;j++)span.bytes.push((meta[at+12+(j>>2)]>>>((j&3)*8))&255);
    }
    for(const span of spans)for(let at=0;at<span.bytes.length;){
      const width=span.bytes.length-at>=4?4:span.bytes.length-at>=2?2:1;
      let value=0;for(let j=0;j<width;j++)value|=span.bytes[at+j]<<(j*8);
      reject(bin(0x47,[...get(34),width===4?0x28:width===2?0x2f:0x2d,width===4?2:width===2?1:0,...uleb(meta[2]+span.ip+at)],c(value)));
      at+=width;
    }
    body.push(...save(28,add(context(28),c(1))));
  }
  body.push(0x03,0x40); // bounded run, including direct back-edge loops
  for(let index=0;index<count;index++){
    const ip=packet[2+index*4],length=packet[3+index*4],encoded=packet[4+index*4],tail=packet[5+index*4];
    const bytes=[encoded&255,(encoded>>>8)&255,(encoded>>>16)&255,encoded>>>24,tail&255,tail>>>8];
    const [op,m]=bytes,wide=!!(op&1),rm=m&7,r=(m>>3)&7;
    const info=meta.subarray(header+index*stride,header+index*stride+12),[exec0,exec1,seg,base,regIndex,disp,width,access,addressConstant,baseMask,policy,fetch]=info;
    const memory=width!==0,displacement=(m>>6)===1?1:(m>>6)===2||(m>>6)===0&&rm===6?2:0;
    const imm=(at,w)=>w?bytes[at]|bytes[at+1]<<8:bytes[at];
    const videoAccess=linked&&memory&&!!(access&3)&&!(access&4);
    const hostAddress=videoAccess?get(49):add(get(34),get(10));
    const loadMemory=()=>[...hostAddress,width===4?0x28:width===2?0x2f:0x2d,width===4?2:width===2?1:0,0];
    const wordCost=address=>add(c(4),add(bin(0x6c,and(address,c(1)),c(3)),shl(bin(0x46,and(address,c(3)),c(3)),2)));
    const writeMemory=value=>[...hostAddress,...value,width===1?0x3a:0x3b,width===1?0:1,0];
    const rmValue=()=>memory?get(11):readReg(rm,wide);
    const writeRm=value=>memory?writeMemory(value):writeReg(rm,wide,value);
    const writeFlags=value=>set(7,value);
    exitIf(bin(0x4f,get(28),get(36))); // committed >= work cap
    if(op===0xcd&&(access&32)){
      if(!bios)throw Error('Missing compound BIOS context');
      // All guards precede the first push. A failed guard leaves the original
      // INT available to the interpreter, including partial work/event budgets.
      exitIf(bin(0x4b,add(get(28),c(3)),get(36)));
      exitIf(not(context(23)));
      exitIf(bin(0x47,readReg(4,false),c(4)));
      exitIf(bin(0x49,readReg(4,true),c(6)));
      body.push(...set(10,add(get(39+seg),sub(readReg(4,true),c(6)))));
      exitIf(bin(0x4b,get(10),c(0x9fffa)));
      exitIf(and(bin(0x49,get(10),get(38)),bin(0x4b,add(get(10),c(6)),get(37))));
      const entry=bytes[1]*4;
      exitIf(bin(0x49,context(21),c(entry+3)));
      exitIf(bin(0x4b,context(20),c(0x9fffc-entry)));
      body.push(...set(49,add(context(20),c(entry))));
      const at=(value,opcode,align=0)=>[...add(get(34),value),opcode,align,0];
      exitIf(bin(0x47,at(get(49),0x28,2),c(0xf0001060)));
      exitIf(bin(0x47,at(c(0xf1060),0x2f,1),c(0xf0e6)));
      exitIf(bin(0x47,at(c(0xf1062),0x2d),c(0xcf)));
      body.push(...set(50,add(add(wordCost(get(10)),wordCost(add(get(10),c(2)))),wordCost(add(get(10),c(4))))),
        ...set(51,add(wordCost(get(49)),wordCost(add(get(49),c(2))))),
        ...set(13,add(c(fetch+8),add(get(51),shl(get(50),1)))));
      exitIf(bin(0x4b,add(get(27),add(c(exec0),get(13))),get(35)));
      const push=(offset,value)=>[...add(get(34),get(10)),...value,0x3b,1,offset];
      body.push(...push(4,get(7)),...push(2,context(22)),...push(0,c(ip+2)));
      // Read after the writes: the saved frame may alias the keyboard bitmap.
      body.push(...set(11,[...bin(0x49,readReg(0,false),c(16)),0x04,0x7f,
        ...at(add(c(0x52a),readReg(0,false)),0x2d),0x05,...c(0),0x0b]),
        ...writeReg(4,false,get(11)),...set(7,or(and(get(7),c(0x7fd5)),c(2))),
        ...set(33,c(ip+2)),...set(32,bin(0x47,get(11),c(4))),...set(25,c(16)),
        ...set(27,add(get(27),add(c(exec0),get(13)))),...set(28,add(get(28),c(3))),
        ...set(29,c(exec1)),...set(30,add(c(4),get(50))),...set(31,c(0)),
        ...set(45,c(index)),...(linked?set(52,c(packet[0])):[]),...set(46,c(0)),...set(47,c(5)),...set(48,c(1)));
      continue;
    }
    if(memory){
      if(base<8)used.add(base);if(regIndex<8)used.add(regIndex);
      body.push(...set(10,and(add(add(base<8?get(15+base):c(0),regIndex<8?get(15+regIndex):c(0)),c(disp)),c(65535))));
      if(access&3){
      exitIf(bin(0x4b,get(10),c(65536-width)));
      body.push(...set(10,add(get(39+seg),get(10))));
      if(videoAccess){
        const within=(lo,hi)=>and(bin(0x4f,get(10),c(lo)),bin(0x4d,get(10),c(hi-width)));
        // Only plain planar windows have a direct host pointer. Test the
        // complete physical operand before any read, store or retirement.
        body.push(...set(49,add(get(34),get(10))),...set(50,bin(0x4b,get(10),c(0xa0000-width))),
          ...get(50),0x04,0x40,
            ...set(51,within(0xa8000,0xc0000)),
            ...or(not(context(33)),not(or(get(51),within(0xe0000,0xe8000)))),0x0d,2,
            ...set(49,add(context(33),[...sub(get(10),c(0xa8000)),...sub(get(10),c(0xc8000)),...get(51),0x1b])),
          0x0b);
      }else exitIf(bin(0x4b,get(10),c(0xa0000-width)));
      if(access&2)exitIf(and(bin(0x49,get(10),get(38)),bin(0x4b,add(get(10),c(width)),get(37))));
      }
    }
    const transfers=Number(!!(access&1))+Number(!!(access&2));
    // A far pointer is fetched as two ordered word accesses.
    const ramPenalty=width===4?add(wordCost(get(10)),wordCost(add(get(10),c(2)))):width===2?wordCost(get(10)):c(4);
    // The planar bus transfers one aligned 16-bit beat at eight clocks.
    // Unaligned words add three clocks and cross to a second beat.
    const videoPenalty=width===1?c(8):add(c(width*4),bin(0x6c,and(get(10),c(1)),c(width/2*11)));
    const dataPenalty=videoAccess?[...videoPenalty,...ramPenalty,...get(50),0x1b]:ramPenalty;
    body.push(...set(13,transfers?add(c(fetch),bin(0x6c,c(transfers),dataPenalty)):c(fetch)),
      ...set(26,add(c(addressConstant),nz(and(get(25),c(baseMask))))));
    exitIf(bin(0x4b,add(get(27),add(get(13),add(get(26),c(Math.max(exec0,exec1))))),get(35)));
    if(memory&&(access&1))body.push(...set(11,loadMemory()));
    // A RAM peek has no bus effects. Reject a mapped return target before
    // committing SP/IP or charging the instruction; the original RET resumes.
    if(op===0xc3){
      const target=add(get(40),get(11));
      // The root already checked A20. A near return keeps the code segment
      // and must stay in the same executable RAM range as the trace.
      exitIf(highCode ? or(bin(0x49,target,c(0x100000)),bin(0x4f,target,c(0x110000)))
        : bin(0x4f,target,c(0xa0000)));
    }
    const destinations=policy===255?Array.from({length:8},(_,r)=>r).filter(r=>writtenRegisters(op,m)&(1<<r)):[];
    if(destinations.length>2)throw Error('RAM block register policy');
    for(let n=0;n<destinations.length;n++){used.add(destinations[n]);body.push(...set(23+n,get(15+destinations[n])));}
    body.push(...set(9,c(0)));
    if(op<=0x3d&&(op&7)<=5){
      const kind=op>>3,direct=(op&7)>=4,dst=direct?0:op&2?r:rm,memDst=!direct&&!(op&2)&&memory;
      body.push(...arithmetic(kind,wide,dst,memDst?rmValue():readReg(dst,wide),direct?c(imm(1,wide)):op&2?rmValue():readReg(r,wide),{
        write:kind!==7,writeFlags,putRegister:(dst,w,v)=>memDst?writeMemory(v):writeReg(dst,w,v)}));
    }else if(op>=0x40&&op<=0x4f){
      body.push(...arithmetic(op&8?5:0,true,op&7,readReg(op&7,true),c(1),{increment:true,writeFlags,putRegister:writeReg}));
    }else if([0x06,0x0e,0x16,0x1e].includes(op)){body.push(...writeMemory(selector(op>>3)),...writeReg(4,true,sub(readReg(4,true),c(2))));
    }else if(op>=0x50&&op<=0x57){body.push(...writeMemory(readReg(op&7,true)),...writeReg(4,true,sub(readReg(4,true),c(2))));
    }else if(op>=0x58&&op<=0x5f){body.push(...writeReg(4,true,add(readReg(4,true),c(2))),...writeReg(op&7,true,get(11)));
    }else if(op===0xc3){body.push(...writeReg(4,true,add(readReg(4,true),c(2))));
    }else if(op>=0x70&&op<=0x7f){body.push(...set(9,condition(op&15)));
    }else if([0x80,0x81,0x83].includes(op)){
      const value=op===0x83?(bytes[2+displacement]<<24)>>24:imm(2+displacement,wide);
      body.push(...arithmetic(r,wide,rm,rmValue(),c(value),{write:r!==7,writeFlags,putRegister:(_r,_w,v)=>writeRm(v)}));
    }else if(op===0x84||op===0x85){body.push(...arithmetic(4,wide,rm,rmValue(),readReg(r,wide),{write:false,writeFlags}));
    }else if(op===0x86||op===0x87){body.push(...set(4,rmValue()),...writeRm(readReg(r,wide)),...writeReg(r,wide,get(4)));
    }else if(op>=0x88&&op<=0x8b){body.push(...(op&2?writeReg(r,wide,rmValue()):writeRm(readReg(r,wide))));
    }else if(op===0x8c){body.push(...writeReg(rm,true,selector(r)));
    }else if(op===0x8e||op===0x07||op===0x1f||op===0xc4||op===0xc5){
      if(!linked)throw Error('Missing linked segment context');
      const pointer=op===0xc4||op===0xc5;
      const segment=op===0x8e?r:op===0xc4?0:op===0xc5?3:op>>3;
      const value=pointer?bin(0x76,get(11),c(16)):op===0x8e&&!memory?readReg(rm,true):get(11);
      // The completed load is visible to subsequent instructions and linked
      // traces through the context. Rust commits the changed caches on exit.
      body.push(...store(14,116+2*segment,2,value),
        ...set(39+segment,shl(selector(segment),4)),...save(12+segment,get(39+segment)),
        ...save(32,or(context(32),c(1<<segment))));
      if(pointer)body.push(...writeReg(r,true,get(11)));
      else if(op!==0x8e)body.push(...writeReg(4,true,add(readReg(4,true),c(2))));
    }else if(op===0x8d){body.push(...writeReg(r,true,get(10)));
    }else if(op>=0x90&&op<=0x97){body.push(...set(4,readReg(0,true)),...writeReg(0,true,readReg(op&7,true)),...writeReg(op&7,true,get(4)));
    }else if(op===0x9e){body.push(...writeFlags(or(or(and(get(7),c(~0xd5)),and(readReg(4,false),c(0xd5))),c(2))));
    }else if(op===0x9f){body.push(...writeReg(4,false,get(7)));
    }else if(op>=0xa0&&op<=0xa3){body.push(...(op&2?writeMemory(readReg(0,wide)):writeReg(0,wide,get(11))));
    }else if(op===0xa8||op===0xa9){body.push(...arithmetic(4,wide,0,readReg(0,wide),c(imm(1,wide)),{write:false,writeFlags}));
    }else if(op>=0xb0&&op<=0xbf){const w=op>=0xb8;body.push(...writeReg(op&7,w,c(imm(1,w))));
    }else if([0xc0,0xc1,0xd0,0xd1].includes(op)){
      body.push(...constantShift(r,wide,rm,op<0xd0?imm(2+displacement,false)&31:1,writeFlags,()=>rmValue(),(_r,_w,v)=>writeRm(v),0x8d5));
    }else if(op===0xc6||op===0xc7){body.push(...writeRm(c(imm(2+displacement,wide))));
    }else if(op>=0xe0&&op<=0xe3){
      if(op!==0xe3)body.push(...writeReg(1,true,sub(readReg(1,true),c(1))));
      let take=op===0xe3?not(readReg(1,true)):nz(readReg(1,true));
      if(op===0xe0||op===0xe1)take=and(take,op===0xe0?not(flag(0x40)):nz(flag(0x40)));
      body.push(...set(9,take));
    }else if(op===0xe8||op===0xe9||op===0xeb){
      if(op===0xe8)body.push(...writeMemory(c((ip+length)&65535)),...writeReg(4,true,sub(readReg(4,true),c(2))));
    }else if(op===0xf6||op===0xf7){
      if(r===0)body.push(...arithmetic(4,wide,rm,rmValue(),c(imm(2+displacement,wide)),{write:false,writeFlags}));
      else if(r===2)body.push(...writeRm(xor(rmValue(),c(-1))));
      else if(r===3)body.push(...arithmetic(5,wide,rm,c(0),rmValue(),{writeFlags,putRegister:(_r,_w,v)=>writeRm(v)}));
      else throw Error('Unsupported RAM group');
    }else if(op===0xfe||op===0xff){body.push(...arithmetic(r?5:0,wide,rm,rmValue(),c(1),{increment:true,writeFlags,putRegister:(_r,_w,v)=>writeRm(v)}));
    }else if([0xf5,0xf8,0xf9,0xfc,0xfd].includes(op)){
      const bits=op>=0xfc?1024:1;body.push(...writeFlags(op===0xf5?xor(get(7),c(1)):op===0xf8||op===0xfc?and(get(7),c(~bits)):or(get(7),c(bits))));
    }else throw Error('Unsupported RAM opcode');
    const branch=op>=0x70&&op<=0x7f||op>=0xe0&&op<=0xe3||op===0xe8||op===0xe9||op===0xeb;
    const unconditional=op===0xe8||op===0xe9||op===0xeb;
    let target;
    if(branch){const offset=op===0xe8||op===0xe9?(imm(1,true)<<16)>>16:(bytes[1]<<24)>>24;target=(ip+length+offset)&65535;}
    body.push(...set(33,op===0xc3?get(11):unconditional?c(target):branch?[...c(target),...c(ip+length),...get(9),0x1b]:c(ip+length)),
      ...set(32,get(25)),...set(25,policy===255?destinations.map((r,n)=>shl(nz(xor(get(15+r),get(23+n))),r)).reduce(or,c(0)):c(policy)),
      ...set(12,add(exec0===exec1?c(exec0):[...c(exec1),...c(exec0),...get(9),0x1b],get(26))),
      ...set(27,add(get(27),add(get(12),get(13)))),...set(28,add(get(28),c(1))),
      ...set(29,get(12)),...set(30,get(13)),...set(31,get(26)),...set(45,c(index)),...(linked?set(52,c(packet[0])):[]),...set(46,get(9)),...(bios||linked?set(48,c(0)):[]));
    // Commit a conditional/return with its actual IP. A different successor
    // exits before any operation on the decoded fallthrough or caller path.
    if((op===0xc3||branch&&!unconditional)&&index+1<count)
      body.push(...bin(0x47,get(33),c(packet[2+(index+1)*4])),0x0d,1);
    if(index+1===count&&branch&&target===packet[2])body.push(...(unconditional?[0x0c,0]:[...get(9),0x0d,0]));
  }
  body.push(0x0c,1,0x0b,0x0b);
  for(const r of dirty)body.push(...store(0,r*2,2,get(15+r)));
  body.push(...storeFlags(get(7)),...get(2),...get(33),0x36,2,0,
    ...save(3,get(28)),...save(4,get(27)),...save(5,get(25)),...save(6,get(29)),...save(7,get(30)),...save(8,get(31)),...save(9,get(32)),...save(18,get(45)),...save(19,get(46)),...(bios||linked?[...save(24,get(47)),...save(25,get(48))]:[]),...(linked?save(26,get(52)):[]),0x10,0,0x0b);
  // Potentially dirty registers also need their entry value: an early exit
  // can flush before their first write. Unused, never-written registers stay in CPU memory.
  body.splice(reloadAt,0,...[...used].sort((a,b)=>a-b).flatMap(r=>set(15+r,load(0,r*2,2))));
  const types=[2,0x60,0,0,0x60,4,0x7f,0x7f,0x7f,0x7f,0];
  const imports=[2,...text('core'),...text('finish'),0,0,...text('core'),...text('memory'),2,0,0];
  return new Uint8Array([0,97,115,109,1,0,0,0,...section(1,types),...section(2,imports),...section(3,[1,1]),...section(7,[1,...text('run'),0,1]),...section(10,[1,...uleb(body.length),...body])]);
}
