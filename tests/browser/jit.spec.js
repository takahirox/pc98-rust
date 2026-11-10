import {test,expect,inspect} from './helpers/runtime.js';

test('default production Worker JIT matches interpreter clocks, registers, pixels and PCM at both roots',async({page})=>{
  const results=[];
  for(const mode of ['', '&jit=1', '&jit=0']){
    const enabled=mode!=='&jit=0';
    await page.goto(`./?diagnostics&manual${mode}`);
    await expect(page.locator('#profile')).toBeEnabled();
    const disk=Buffer.alloc(1261568);
    // Original MIT register/call loop, including stack, TEST/NOT/NEG and carry-dependent SHR/RCL.
    // CLI keeps external input outside this fixed-work probe.
    disk.set([0xfa,0xbc,0,0x80,0x01,0xd8,0xe8,11,0,0x31,0xc1,0x46,0x4a,0x09,0xc3,0x83,0xc7,1,0xeb,0xf0,
      0x51,0xf6,0xc4,0x81,0xf7,0xd5,0xf7,0xdd,0xd0,0xe8,0xd1,0xd3,0x59,0xc3]);
    await page.locator('#drive').selectOption('0');
    await page.locator('#file').setInputFiles({name:'jit.hdm',mimeType:'application/octet-stream',buffer:disk});
    await expect(page.locator('#file')).toBeEnabled();
    await inspect(page,()=>pc98.core.set_boot_source(0));await page.locator('#reset').click();
    for(let n=0;n<24;n++)await inspect(page,()=>pc98.core.step_scheduler(80000,10000));
    const result=await inspect(page,()=>{
      const core=pc98.core,ptr=core.jit_layout()>>>0;
      const [r,f,ip]=new Uint32Array(core.memory.buffer,ptr,3),view=new DataView(core.memory.buffer);
      const hash=bytes=>{let h=2166136261;for(const b of bytes)h=Math.imul(h^b,16777619)>>>0;return h;};
      const frame=core.frame()>>>0;
      const frameHash=hash(new Uint8Array(core.memory.buffer,frame,640*400*4));
      const samples=core.audio_len(),audio=core.audio_ptr()>>>0;
      const pcmHash=hash(new Uint8Array(core.memory.buffer,audio,samples*4));
      return {state:{r:Array.from(new Uint16Array(core.memory.buffer,r,8)),flags:view.getUint16(f,true),ip:view.getUint32(ip,true),
        metrics:[0,1,2,5,19,20,26,27,28,30].map(i=>core.metric(i)),frameHash,pcmHash,samples,error:pc98.stats().error},jit:pc98.stats().jit};
    });
    expect(result.state.error).toBe('');
    if(enabled){expect(result.jit.compiled).toBeGreaterThan(0);expect(result.jit.instructions).toBeGreaterThan(10000);expect(result.jit.failures).toBe(0);}
    else expect(result.jit.mode).toBe('interpreter');
    results.push(result.state);
    await page.locator('#reset').click();
    if(enabled){const stats=await page.evaluate(()=>pc98.stats());expect(stats.jit.instructions).toBe(0);expect(stats.jit.cached).toBe(0);}
  }
  expect(results[0]).toEqual(results[1]);
  expect(results[0]).toEqual(results[2]);
});

test('an unavailable JIT module retains a working interpreter',async({page},testInfo)=>{
  test.skip(testInfo.project.name==='pages','Failure path is covered once; both asset roots are covered above.');
  let requests=0;
  await page.route('**/jit.js',route=>{requests++;return route.abort();});
  await page.goto('./?diagnostics&manual&jit=0');await expect(page.locator('#profile')).toBeEnabled();
  expect((await page.evaluate(()=>pc98.stats())).jit).toEqual({mode:'interpreter',unavailable:false});
  expect(requests).toBe(0);
  await page.goto('./?diagnostics&manual');await expect(page.locator('#profile')).toBeEnabled();
  const result=await inspect(page,()=>{
    pc98.core.step_scheduler(80000,10000);return pc98.stats();
  });
  expect(result.jit).toEqual({mode:'interpreter',unavailable:true});
  expect(requests).toBe(1);
  expect(result.completedInstructions).toBeGreaterThan(0);expect(result.error).toBe('');
  await expect(page.locator('#error')).toBeHidden();
});
