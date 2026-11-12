// Opt-in measurement helpers. No emulator state or browser automation lives here.
export const image='mcr.microsoft.com/playwright:v1.56.1-noble';
export const executable='/ms-playwright/chromium-1194/chrome-linux/chrome';
export const period=4000; // Linux quota minimum is 1 ms; keep periods comparable.

export function budgetOptions(value){
  if(value==='unlimited')return {label:value,quota:-1,period};
  if(!/^(?:0\.[0-9]+|1(?:\.[0-9]+)?|2(?:\.0+)?)$/.test(value))throw Error('Expected unlimited or a CPU budget from 0.25 to 2');
  const cpus=Number(value),quota=cpus*period;
  if(cpus<0.25||cpus>2||!Number.isInteger(quota))throw Error('CPU budget is outside the supported range');
  return {label:value,quota,period};
}

export function expectedCalibration(count){
  // x[n+1] = 3*x[n]+1, x[0]=1. Compute independently of the Wasm loop.
  let exponent=BigInt(count)+1n,base=3n,power=1n;
  const modulus=1n<<33n;
  while(exponent){if(exponent&1n)power=power*base%modulus;base=base*base%modulus;exponent>>=1n;}
  return Number((power-1n)/2n)>>>0;
}

export function calibrationModule(){
  const leb=n=>{const a=[];do{const b=n&127;n>>>=7;a.push(b|(n?128:0));}while(n);return a;};
  const section=(id,bytes)=>[id,...leb(bytes.length),...bytes];
  const body=[1,1,0x7f,0x41,1,0x21,1,0x02,0x40,0x03,0x40,
    0x20,0,0x45,0x0d,1,0x20,1,0x41,3,0x6c,0x41,1,0x6a,0x21,1,
    0x20,0,0x41,1,0x6b,0x21,0,0x0c,0,0x0b,0x0b,0x20,1,0x0b];
  return new Uint8Array([0,97,115,109,1,0,0,0,...section(1,[1,0x60,1,0x7f,1,0x7f]),
    ...section(3,[1,0]),...section(7,[1,3,114,117,110,0,0]),...section(10,[1,...leb(body.length),...body])]);
}

export function percentile(values,fraction){
  if(!values.length)return null;
  const sorted=[...values].sort((a,b)=>a-b);
  return sorted[Math.max(0,Math.ceil(sorted.length*fraction)-1)];
}

export function playbackSummary(rows){
  if(rows.length<2)throw Error('Playback has no measurable interval');
  const first=rows[0],last=rows.at(-1),wallMs=last.wallMs-first.wallMs;
  if(!(wallMs>0))throw Error('Invalid playback interval');
  const fields=['wallMs','guestSeconds','frames','underflows','overflows','droppedSamples','sequenceErrors','checksumErrors','guestSlices','guestSliceMs','hddReads','fmKeyEdges'];
  if(rows.some(r=>fields.some(k=>!Number.isFinite(r[k])||r[k]<0)))throw Error('Missing or invalid playback counters');
  if(rows.some(r=>r.error||r.hidden||r.running===false||(r.audioState&&r.audioState!=='running')||r.audioEpoch!==first.audioEpoch||r.rendererFailure))throw Error('Playback stopped, flushed, hidden or faulted');
  if(fields.some(k=>rows.some((r,i)=>i&&r[k]<rows[i-1][k])))throw Error('Playback counters moved backwards');
  const delta=key=>last[key]-first[key];
  const sumDelta=(group,key)=>(last[group]?.[key]??0)-(first[group]?.[key]??0);
  const slices=delta('guestSlices'),snapshots=sumDelta('videoStats','snapshots');
  return {wallMs,guestRatio:delta('guestSeconds')*1000/wallMs,fps:delta('frames')*1000/wallMs,
    missingSamples:delta('underflows'),overflows:delta('overflows'),drops:delta('droppedSamples'),
    sequenceErrors:delta('sequenceErrors'),checksumErrors:delta('checksumErrors'),
    sliceMeanMs:slices?delta('guestSliceMs')/slices:null,
    snapshotMeanMs:snapshots?sumDelta('videoStats','snapshotMs')/snapshots:null,
    rasterMeanMs:snapshots?sumDelta('renderStats','rasterMs')/snapshots:null,
    copyMeanMs:snapshots?(sumDelta('videoStats','copyMs')+sumDelta('renderStats','copyMs'))/snapshots:null,
    reads:delta('hddReads'),keys:delta('fmKeyEdges'),error:last.error,
    videoMode:last.videoMode??null,
    deviceSampleRate:last.deviceSampleRate??null,
    deviceClockRatio:last.deviceSampleRate?((delta('scheduledSamples')+delta('underflows'))/last.deviceSampleRate)*1000/wallMs:null,
    translatedFraction:delta('completedInstructions')>0?sumDelta('jit','instructions')/delta('completedInstructions'):null,
    compilations:sumDelta('jit','compiled'),compilationWallMs:sumDelta('jit','compilationMs'),
    sampleGapP99Ms:percentile(rows.slice(1).map((r,i)=>r.wallMs-rows[i].wallMs),0.99)};
}
