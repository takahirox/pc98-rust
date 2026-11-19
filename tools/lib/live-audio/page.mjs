import {Presenter} from './presenter.js';
const params=new URL(location.href).searchParams;
const reference=params.get('reference')==='1', reserve=Number(params.get('reserve'));
const raw=document.querySelector('#raw'),crt=document.querySelector('#crt');
const presenter=new Presenter(raw,crt,document.querySelector('#surface'));
const audio=new AudioContext({sampleRate:48000});
const worker=new Worker('live-worker.mjs',{type:'module'});
const waiters=new Map();
let fault,frames=0,rafTime=0,cbGap=0;
const reject=error=>{fault=error;for(const cb of waiters.values())cb.reject(error);waiters.clear();};
worker.onerror=e=>reject(Error(e.message));
worker.onmessage=({data:m})=>{
  if(m.type==='fault'){reject(Error(m.error));return;}
  if(m.type==='frame'){
    requestAnimationFrame(t=>{
      if(rafTime)cbGap=Math.max(cbGap,t-rafTime);
      rafTime=t;frames++;
      if(m.changed)presenter.present(new ImageData(new Uint8ClampedArray(m.buffer),640,400));
      worker.postMessage({type:'frameAck',buffer:m.buffer},[m.buffer]);
    });
    return;
  }
  const k=m.id??m.type;const cb=waiters.get(k);if(cb){waiters.delete(k);cb.resolve(m);}
};
const get=(id,send)=>new Promise((resolve,reject)=>{
  if(fault){reject(fault);return;}
  waiters.set(id,{resolve,reject});send();
});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const samples=[];
let begin,id=0;
async function sample(phase){
  const seq=++id;
  const stats=await get(seq,()=>worker.postMessage({type:'sample',id:seq}));
  const output=audio.playbackStats, legacy=audio.playoutStats;
  samples.push({...stats,phase,wallMs:performance.now()-begin,deviceTime:audio.currentTime,frames,
    uploads:presenter.stats.uploads,crtActive:presenter.diagnosticsState().active,cbGap,
    device:output?{total:output.totalDuration,underrun:output.underrunDuration,events:output.underrunEvents}:legacy?{total:legacy.totalFramesDuration/1000,underrun:legacy.fallbackFramesDuration/1000,events:legacy.fallbackFramesEvents}:null});
}
const mouse=(...args)=>worker.postMessage({type:'mouse',args});
try{
  await audio.suspend();await audio.audioWorklet.addModule('./audio-worklet.js');
  const node=new AudioWorkletNode(audio,'pc98-audio',{outputChannelCount:[1]});node.connect(audio.destination);
  const link=new MessageChannel();node.port.postMessage({type:'connect',port:link.port1},[link.port1]);
  await get('ready',()=>worker.postMessage({type:'init',reference,reserve,port:link.port2},[link.port2]));
  begin=performance.now();worker.postMessage({type:'start'});await audio.resume();
  // Exclude the bounded warm-up from both measurement windows.
  await sleep(3000);
  for(let i=0;i<=24;i++){await sample('stationary');if(i<24)await sleep(1000);}
  mouse(-20,100,0);
  for(let i=0;i<48;i++){
    await sample('progressing');mouse(0,0,1);await sleep(100);mouse(0,0,0);await sleep(500);
  }
  await sample('progressing');
  const end=await get('finish',()=>worker.postMessage({type:'finish'}));
  await audio.close();worker.terminate();
  await fetch('/result',{method:'POST',body:JSON.stringify({userAgent:navigator.userAgent,reference,sampleRate:audio.sampleRate,baseLatency:audio.baseLatency,outputLatency:audio.outputLatency,finalAnchor:end.anchor,samples})});
}catch(error){
  worker.terminate();await fetch('/result',{method:'POST',body:JSON.stringify({error:String(error),samples})});
}
