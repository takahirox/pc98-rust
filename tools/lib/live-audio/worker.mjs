const pixelBytes = 640 * 400 * 4;
const read = async name => {
  const response = await fetch(name);
  if (!response.ok) throw Error('Missing '+name);
  return new Uint8Array(await response.arrayBuffer());
};
const sleep = ms => new Promise(resolve => setTimeout(resolve,ms));
const anchor = pixels => {
  let hash=2166136261;
  for(let y=326;y<348;y++) for(let x=118;x<338;x++) {
    const i=(y*640+x)*4;
    hash=Math.imul(hash^(Math.min(pixels[i],pixels[i+1],pixels[i+2])>200?1:0),16777619)>>>0;
  }
  return hash;
};
async function own(disk){
  const {BlockJit}=await import('./block-jit.js');
  const module=await WebAssembly.compile(await read('core.wasm'));
  const c=new WebAssembly.Instance(module).exports,r=new WebAssembly.Instance(module).exports;
  const put=(core,bytes)=>{
    const ptr=core.input_buffer(bytes.length)>>>0;
    new Uint8Array(core.memory.buffer,ptr,bytes.length).set(bytes);
  };
  c.set_profile(1);c.set_boot_source(1);put(c,disk.subarray(0,32));
  if(!c.hdi_begin(disk.length))throw Error('HDI begin');
  for(let at=0;at<disk.length;at+=1048576){put(c,disk.subarray(at,at+1048576));if(!c.hdi_append())throw Error('HDI append');}
  if(!c.hdi_commit())throw Error('HDI commit');
  c.reset();const j=new BlockJit(c);
  function step(clocks=80000){
    j.step(clocks,100000);
    if(c.metric(4)||j.failed)throw Error('Guest or JIT failed');
    return new Float32Array(c.memory.buffer,c.audio_ptr()>>>0,c.audio_len());
  }
  async function advance(ms){
    const target=c.metric(5)+ms*8000;
    while(c.metric(5)<target){
      step(Math.ceil(Math.min(80000,target-c.metric(5))));c.audio_clear();
      while(j.pending)await sleep(0);
    }
  }
  const scans={' ':52,'2':2};
  for(const [letters,start] of [['qwertyuiop',16],['asdfghjkl',29],['zxcvbnm',41]])
    for(let i=0;i<letters.length;i++)scans[letters[i]]=start+i;
  async function key(scan){c.key(scan,1);await advance(55);c.key(scan,0);await advance(20);}
  async function command(s){for(const letter of s)await key(scans[letter]);await key(28);await advance(1000);}
  async function mouse(x,y,buttons,ms){c.mouse(x,y,buttons);await advance(ms);}
  await advance(12000);await command('cd nanpa2');await command('nanpa2');await key(28);await advance(1000);
  for(let i=0;i<4;i++)await advance(10000);
  for(let i=0;i<8;i++)await mouse(-100,-100,0,30);
  for(let i=0;i<5;i++)await mouse(100,50,0,30);
  await mouse(20,0,1,100);await mouse(0,0,0,1000);await advance(10000);
  const pixels=()=>{
    const ptr=c.video_snapshot()>>>0;
    put(r,new Uint8Array(c.memory.buffer,ptr,c.video_snapshot_len()));
    const out=r.video_render(c.video_snapshot_len())>>>0;
    if(!out)throw Error('Snapshot raster');
    return new Uint8Array(r.memory.buffer,out,pixelBytes);
  };
  return {step,clear:c.audio_clear,seconds:()=>c.metric(29),mouse:c.mouse,pixels,changed:()=>!!r.video_render_changed(),reads:()=>c.metric(9)};
}
async function reference(disk){
  const {default:create}=await import('./np2.mjs');
  const messages=[];
  const m=await create({noInitialRun:true,print:s=>{if(messages.length===8)messages.shift();messages.push(s);},
    printErr:s=>{if(messages.length===8)messages.shift();messages.push(s);}});
  for(const dir of ['/run','/run/system','/run/system/np2kai'])m.FS.mkdir(dir);
  m.FS.writeFile('/run/game.hdi',disk,{canOwn:true});
  m.FS.writeFile('/run/system/np2kai/FONT.ROM',await read('FONT.ROM'));
  m.FS.writeFile('/run/system/np2kai/np2kai.cfg',await read('np2kai.cfg'));
  const status=m.callMain(['static','/run','/run/game.hdi','live']);
  if(status!==0)throw Error('NP2kai setup status '+status+': '+messages.slice(-5).join(' | '));
  return {
    step:()=>{
      m._compare_live_step();
      return new Float32Array(m.HEAPF32.buffer,m._compare_live_pcm(),m._compare_live_pcm_count());
    },
    clear:()=>{},seconds:m._compare_live_seconds,mouse:m._compare_live_mouse,
    pixels:()=>{
      const ptr=m._compare_live_rgba();
      if(!ptr)throw Error('Unsupported reference framebuffer');
      return new Uint8Array(m.HEAPU8.buffer,ptr,pixelBytes);
    },
  };
}
const task=new MessageChannel();
const buffers=Array.from({length:6},()=>new ArrayBuffer(4096));
let machine,pcm,timer,started=false,sink={},sent=0,produced=0,dropped=0,frames=0,frameCredit=new ArrayBuffer(pixelBytes),nextFrame=0;
// Both implementations share the same bounded audio pump and presentation credit.
// In the reference, one native libretro frame is the minimum unit of work.
function schedule(immediate){
  if(!started)return;
  if(immediate&&typeof timer==='number'){clearTimeout(timer);timer=null;}
  if(timer)return;
  if(immediate){timer=true;task.port2.postMessage(null);}else timer=setTimeout(tick,5);
}
function ready(){return buffers.length&&sent-(sink.consumedSamples??0)<2880;}
function tick(){
  timer=null;
  try{
    if(ready()){
      const samples=machine.step(); const start=produced;
      produced+=samples.length;
      for(let at=0;at<samples.length;at+=1024){
        const buffer=buffers.pop(),count=Math.min(samples.length-at,1024);
        if(!buffer){dropped+=samples.length-at;break;}
        new Float32Array(buffer,0,count).set(samples.subarray(at,at+count));
        pcm.postMessage({type:'pcm',epoch:0,buffer,count,start:start+at},[buffer]);sent+=count;
      }
      machine.clear();
    }
    const now=performance.now();
    if(frameCredit&&now>=nextFrame){
      nextFrame=now+1000/56.4;
      const pixels=machine.pixels();
      // Reference has no raster dirty flag. Compare exact pixels against the
      // returned buffer for this shared presentation test.
      const old=new Uint32Array(frameCredit);
      let changed=frames===0 || (machine.changed?.() ?? false);
      if(!changed&&!machine.changed){
        const current=new Uint32Array(pixels.buffer,pixels.byteOffset,old.length);
        for(let at=0;at<old.length;at++)if(old[at]!==current[at]){changed=true;break;}
      }
      if(changed)new Uint8Array(frameCredit).set(pixels);
      frames++;
      postMessage({type:'frame',buffer:frameCredit,changed},[frameCredit]);frameCredit=null;
    }
    schedule(ready());
  }catch(error){started=false;postMessage({type:'fault',error:String(error)});}
}
task.port1.onmessage=tick;
onmessage=async ({data:m})=>{
  try{
    if(m.type==='init'){
      machine=await (m.reference?reference:own)(await read('game.hdi'));
      const initial=anchor(machine.pixels());
      if(initial!==854969663)throw Error('Starting scene anchor mismatch: '+initial);
      pcm=m.port;pcm.onmessage=({data})=>{
        if(data.buffer)buffers.push(data.buffer);
        if(data.stats)sink=data.stats;
        if(ready())schedule(true);
      };
      postMessage({type:'ready'});
    }else if(m.type==='start'){started=true;schedule(true);}
    else if(m.type==='frameAck')frameCredit=m.buffer;
    else if(m.type==='mouse')machine.mouse(...m.args);
    else if(m.type==='sample')postMessage({type:'sample',id:m.id,seconds:machine.seconds(),reads:machine.reads?.(),sent,produced,dropped,frames,sink});
    else if(m.type==='finish'){
      started=false;clearTimeout(timer);timer=null;
      postMessage({type:'finish',anchor:anchor(machine.pixels())});
    }
  }catch(error){postMessage({type:'fault',error:String(error)});}
};
