import {test,expect,inspect} from './helpers/runtime.js';

const stats = page => page.evaluate(()=>pc98.stats());
const rawHash = page => page.locator('#screen').evaluate(async canvas => [...new Uint8Array(await crypto.subtle.digest('SHA-256',canvas.getContext('2d').getImageData(0,0,640,400).data))]);
async function ready(page) {
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(0);
}

test('live presets preserve raw frames, guest state and input across VM/Ap and context restoration',async({page})=>{
  await ready(page);
  expect(await page.evaluate(()=>localStorage.getItem('pc98-presentation'))).toBeNull();
  await expect(page.locator('#presentation')).toHaveValue('sharp');
  await expect(page.locator('#presentation option[value="sharp"]')).toHaveJSProperty('defaultSelected',true);
  await expect(page.locator('#phosphor-mask')).not.toBeChecked();
  await expect(page.locator('#crt-screen')).toBeVisible();
  await expect(page.locator('#presentation-status')).toContainText('RGB CRT');
  expect((await stats(page)).presenterStats).toMatchObject({preset:'sharp',active:true,fallback:false});
  for (const profile of ['0','1']) {
    await page.locator('#profile').selectOption(profile); await expect(page.locator('#profile')).toBeEnabled();
    if ((await stats(page)).running) await page.locator('#pause').click();
    await expect.poll(()=>stats(page).then(s=>s.frameInFlight)).toBe(false);
    const hash = await rawHash(page), before = await stats(page);
    for (const preset of ['sharp','soft','strong','off','sharp']) {
      await page.locator('#presentation').selectOption(preset);
      expect(await rawHash(page)).toEqual(hash);
      expect((await stats(page)).cycles).toBe(before.cycles);
      expect((await stats(page)).audioEpoch).toBe(before.audioEpoch);
      const ratio = await page.locator('#screen').evaluate(c=>c.clientWidth/c.clientHeight);
      expect(ratio).toBeCloseTo(preset === 'off' ? 1.6 : 4/3,2);
    }
    await expect(page.locator('#crt-screen')).toBeVisible();
    await page.locator('#screen').focus();
    // The overlay never takes keyboard focus or pointer events.
    await expect(page.locator('#screen')).toBeFocused();
    expect(await page.locator('#crt-screen').evaluate(c=>getComputedStyle(c).pointerEvents)).toBe('none');
    await page.locator('#crt-screen').evaluate(c=>{
      window.crtLoss = c.getContext('webgl2').getExtension('WEBGL_lose_context'); crtLoss.loseContext();
    });
    await expect(page.locator('#crt-screen')).toBeHidden();
    await expect(page.locator('#presentation-status')).toContainText('context lost');
    await expect(page.locator('#presentation-status')).toContainText('showing original pixels');
    expect(await rawHash(page)).toEqual(hash);
    await page.evaluate(()=>crtLoss.restoreContext());
    await expect(page.locator('#crt-screen')).toBeVisible();
    expect(await rawHash(page)).toEqual(hash);
    expect((await stats(page)).cycles).toBe(before.cycles);
  }
  await page.locator('#fullscreen').click();
  await expect.poll(()=>page.evaluate(()=>Boolean(document.fullscreenElement || document.querySelector('#drop.full-window')))).toBe(true);
  expect(await page.locator('#screen').evaluate(c=>c.clientWidth/c.clientHeight)).toBeCloseTo(4/3,2);
  await page.locator('#exit-fullscreen').click();
  await expect(page.locator('#exit-fullscreen')).toBeHidden();
  await page.setViewportSize({width:480,height:800});
  expect(await page.locator('#screen').evaluate(c=>c.clientWidth/c.clientHeight)).toBeCloseTo(4/3,2);
  await page.reload(); await expect(page.locator('#profile')).toBeEnabled();
  expect(await page.locator('#presentation').inputValue()).toBe('sharp');
  await expect(page.locator('#crt-screen')).toBeVisible();
  // Exercise actual keyboard delivery with the filtered screen active.
  await page.locator('#hello').click();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
  await page.locator('#game').click();
  await page.locator('#screen').focus(); await page.keyboard.press('ArrowRight');
  expect((await stats(page)).presenterStats.active).toBe(true);
  const live=await stats(page);
  await page.locator('#crt-screen').evaluate(c=>{
    window.liveLoss=c.getContext('webgl2').getExtension('WEBGL_lose_context'); liveLoss.loseContext();
  });
  await expect(page.locator('#crt-screen')).toBeHidden();
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(live.presentationStats.frames+2);
  expect((await stats(page)).generation).toBe(live.generation);
  await page.evaluate(()=>liveLoss.restoreContext());
  await expect(page.locator('#crt-screen')).toBeVisible();
  await expect(page.locator('#error')).toBeHidden();
});

for (const preset of ['off','sharp','soft','strong']) test(`saved ${preset} and mask preferences survive reloads and manual changes`,async({page})=>{
  await page.addInitScript(preset=>{
    if (localStorage.getItem('pc98-presentation') === null)
      localStorage.setItem('pc98-presentation',JSON.stringify({preset,mask:true}));
  },preset);
  await ready(page);
  await expect(page.locator('#presentation')).toHaveValue(preset);
  await expect(page.locator('#phosphor-mask')).toBeChecked();
  expect((await stats(page)).presenterStats).toMatchObject({preset,active:preset!=='off',fallback:false});
  await page.reload(); await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#presentation')).toHaveValue(preset);
  await expect(page.locator('#phosphor-mask')).toBeChecked();
  await page.locator('#phosphor-mask').uncheck();
  // Persist a newly selected Original pixels choice as well as restoring it.
  await page.locator('#presentation').selectOption('off');
  await page.reload(); await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#presentation')).toHaveValue('off');
  await expect(page.locator('#phosphor-mask')).not.toBeChecked();
  await expect(page.locator('#crt-screen')).toBeHidden();
  expect(await page.evaluate(()=>JSON.parse(localStorage.getItem('pc98-presentation')))).toEqual({preset:'off',mask:false});
});

test('invalid or corrupt saved settings use Sharp CRT with the mask off',async({page})=>{
  await ready(page);
  // Cover JSON parse errors, wrong shapes/types, inherited names and bad mask
  // values without multiplying these storage-only branches by guest profiles.
  for (const saved of ['{','null','[]','"off"','{}',
    '{"preset":"unknown","mask":true}','{"preset":"toString","mask":true}',
    '{"preset":["off"],"mask":true}','{"preset":"off","mask":"false"}',
    '{"preset":"off"}']) {
    await page.evaluate(saved=>localStorage.setItem('pc98-presentation',saved),saved);
    await page.reload(); await expect(page.locator('#profile')).toBeEnabled();
    await expect(page.locator('#presentation')).toHaveValue('sharp');
    await expect(page.locator('#phosphor-mask')).not.toBeChecked();
    await expect(page.locator('#crt-screen')).toBeVisible();
  }
});

test('blocked storage still defaults to Sharp CRT and permits live switching',async({page})=>{
  await page.addInitScript(()=>{
    Object.defineProperty(window,'localStorage',{get(){throw new DOMException('Storage blocked','SecurityError');}});
  });
  await ready(page);
  await expect(page.locator('#presentation')).toHaveValue('sharp');
  await expect(page.locator('#phosphor-mask')).not.toBeChecked();
  await expect(page.locator('#crt-screen')).toBeVisible();
  for (const preset of ['off','soft','strong','sharp']) {
    await page.locator('#presentation').selectOption(preset);
    expect((await stats(page)).presenterStats).toMatchObject({preset,active:preset!=='off'});
  }
  await page.locator('#phosphor-mask').check();
  await page.reload(); await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#presentation')).toHaveValue('sharp');
  await expect(page.locator('#phosphor-mask')).not.toBeChecked();
  await expect(page.locator('#crt-screen')).toBeVisible();
  await expect(page.locator('#error')).toBeHidden();
});

test.describe('touch fullscreen fallback',()=>{
  test.use({viewport:{width:390,height:844},hasTouch:true});
  test('rejected fullscreen request leaves a tappable exit and the guest running',async({page})=>{
    await page.addInitScript(()=>{
      Element.prototype.requestFullscreen = () => Promise.reject(new DOMException('Fullscreen unavailable','NotAllowedError'));
    });
    await ready(page);
    await page.locator('#game').tap();
    await expect(page.locator('#game')).toBeEnabled();
    await expect.poll(()=>stats(page).then(s=>s.running)).toBe(true);
    const before=await stats(page);
    await expect(page.locator('#exit-fullscreen')).toBeHidden();
    await page.locator('#fullscreen').tap();
    await expect(page.locator('#drop')).toHaveClass(/full-window/);
    expect(await page.evaluate(()=>document.fullscreenElement)).toBeNull();
    const exit=page.getByRole('button',{name:'Exit full screen',exact:true});
    await expect(exit).toBeVisible();
    const box=await exit.boundingBox();
    expect(box.x).toBeGreaterThanOrEqual(0); expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.x+box.width).toBeLessThanOrEqual(390); expect(box.y+box.height).toBeLessThanOrEqual(844);
    expect(box.height).toBeGreaterThanOrEqual(44);
    await exit.tap();
    await expect(page.locator('#drop')).not.toHaveClass(/full-window/);
    await expect(exit).toBeHidden();
    await expect(page.locator('#screen')).toBeFocused();
    expect(await page.locator('body').evaluate(b=>getComputedStyle(b).overflow)).not.toBe('hidden');
    await expect(page.locator('#pause')).toBeVisible();
    await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(before.presentationStats.frames+2);
    const after=await stats(page);
    expect(after.running).toBe(true); expect(after.generation).toBe(before.generation);
    expect(after.audioEpoch).toBe(before.audioEpoch); expect(after.cycles).toBeGreaterThan(before.cycles);
    await page.locator('#fullscreen').tap();
    await expect(page.locator('#drop')).toHaveClass(/full-window/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#drop')).not.toHaveClass(/full-window/);
    await expect(page.locator('#screen')).toBeFocused();
    await expect(page.locator('#error')).toBeHidden();
  });
});

for (const fault of ['unsupported','compile','link','allocation','render']) test(`GPU ${fault} falls back while presentation credits and keyboard keep working`,async({page})=>{
  await page.addInitScript(fault=>{
    const get = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function(type,...args) {
      if (type === 'webgl2' && fault === 'unsupported') return null;
      const result = get.call(this,type,...args);
      if (type === 'webgl2' && result && fault === 'compile') result.getShaderParameter = () => false;
      if (type === 'webgl2' && result && fault === 'link') result.getProgramParameter = () => false;
      if (type === 'webgl2' && result && fault === 'allocation') result.getError = () => result.OUT_OF_MEMORY;
      if (type === 'webgl2' && result && fault === 'render') result.drawArrays = () => {throw Error('Injected draw failure');};
      return result;
    };
  },fault);
  await ready(page); const before = await stats(page);
  await expect(page.locator('#presentation')).toHaveValue('sharp');
  await expect(page.locator('#crt-screen')).toBeHidden();
  await expect(page.locator('#screen')).toBeVisible();
  expect(await page.locator('#screen').evaluate(c=>c.clientWidth/c.clientHeight)).toBeCloseTo(1.6,2);
  await expect(page.locator('#presentation-status')).toContainText('showing original pixels');
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(before.presentationStats.frames+2);
  expect((await stats(page)).presenterStats.fallback).toBe(true);
  await page.locator('#hello').click();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
  await expect(page.locator('#error')).toBeHidden();
});

test('shader pixels stay upright, retain black/white and single-pixel edges, and suppress undersampled effects',async({page})=>{
  await ready(page);
  const result = await page.evaluate(async()=>{
    const {Presenter} = await import('./presenter.js');
    const surface = document.createElement('div'); surface.style.cssText = 'width:1280px;height:960px';
    const raw = document.createElement('canvas'), output = document.createElement('canvas');
    surface.append(raw,output); document.body.append(surface);
    const p = new Presenter(raw,output,surface);
    const frame = new ImageData(640,400);
    for (let y=0;y<400;y++) for (let x=0;x<640;x++) {
      const i=(y*640+x)*4;
      frame.data.set([y<100?255:0,x===320?255:0,y>=300?255:0,255],i);
    }
    const original = [...frame.data];
    // A standalone Presenter uses the same default even before setPreset is
    // called, and invalid values cannot disable CRT accidentally.
    const defaultPreset=p.preset; p.present(frame);
    const defaultActive=!output.hidden;
    const defaultTuning=Object.fromEntries(['scan','softness','mask'].map(name=>[name,p.gl.getUniform(p.gl.getParameter(p.gl.CURRENT_PROGRAM),p.uniforms[name])]));
    p.setPreset('invalid'); const invalidPreset=p.preset;
    function sample(x,y) {
      const bytes = new Uint8Array(4); p.gl.readPixels(Math.floor(x*output.width),Math.floor((1-y)*output.height),1,1,p.gl.RGBA,p.gl.UNSIGNED_BYTE,bytes); return [...bytes];
    }
    p.setPreset('strong'); p.present(frame);
    const top=sample(.1,.1),bottom=sample(.1,.9),black=sample(.1,.5),line=sample(320.5/640,.5),neighbor=sample(319.5/640,.5);
    const stable = () => {
      p.redraw(); const a = new Uint8Array(output.width*output.height*4); p.gl.readPixels(0,0,output.width,output.height,p.gl.RGBA,p.gl.UNSIGNED_BYTE,a); return a;
    };
    const a=stable(),b=stable();
    // At a small size scanlines and mask must have no effect at all.
    surface.style.width='640px'; surface.style.height='480px'; p.setPreset('strong',false);
    const small=stable(); p.setPreset('strong',true); const masked=stable();
    const source=sequence=>({renderer:output,sequence,generation:1,videoEpoch:0});
    const rows=(sequence,first)=>({...source(sequence),changedRows:{after:sequence-1,first,end:first+1}});
    p.present(frame,source(1));
    const modified=new ImageData(frame.data.slice(),640,400);
    for(let x=0;x<640;x++) modified.data.set([100,255,40,255],(217*640+x)*4);
    // Let resize observers settle before priming and compositing the target.
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    p.present(modified,rows(2,217));
    await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
    for(let x=0;x<640;x++) modified.data.set([30,50,80,255],(261*640+x)*4);
    p.present(modified,rows(3,261));
    const partialRaw=p.context.getImageData(0,0,640,400).data;
    const partialCrt=new Uint8Array(output.width*output.height*4);
    p.gl.readPixels(0,0,output.width,output.height,p.gl.RGBA,p.gl.UNSIGNED_BYTE,partialCrt);
    p.present(modified);
    const stableFull=new Uint8Array(partialCrt.length);
    p.gl.readPixels(0,0,output.width,output.height,p.gl.RGBA,p.gl.UNSIGNED_BYTE,stableFull);
    const partialEqual=partialCrt.every((v,i)=>v===stableFull[i]);
    const checkFramebufferStatus=p.gl.checkFramebufferStatus;
    p.gl.checkFramebufferStatus=()=>p.gl.FRAMEBUFFER_UNSUPPORTED;
    surface.style.width='648px'; surface.style.height='486px';
    p.present(frame,source(4));
    p.present(frame,rows(5,217));
    const fullDrawFallback=!output.hidden && p.filtered === null && sample(.1,.1)[0]>220;
    p.gl.checkFramebufferStatus=checkFramebufferStatus;
    surface.style.width='2560px'; surface.style.height='1920px'; p.setPreset('strong',false);
    const largePlain=sample(64.5/640,.1); p.setPreset('strong',true); const largeMask=sample(64.5/640,.1);
    const unchanged=frame.data.every((v,i)=>v===original[i]);
    // Future dimensions should use square pixels, without another 1.2 stretch.
    p.present(new ImageData(320,200));
    const futureAspect=surface.style.getPropertyValue('--crt-aspect');
    surface.remove(); p.observer.disconnect();
    return {defaultPreset,defaultActive,defaultTuning,invalidPreset,top,bottom,black,line,neighbor,unchanged,futureAspect,largePlain,largeMask,stable:a.every((v,i)=>v===b[i]),smallMaskEqual:small.every((v,i)=>v===masked[i]),
      partialEqual,partialRawEqual:partialRaw.every((v,i)=>v===modified.data[i]),fullDrawFallback};
  });
  expect(result.defaultPreset).toBe('sharp'); expect(result.defaultActive).toBe(true); expect(result.invalidPreset).toBe('sharp');
  expect(result.defaultTuning.scan).toBeCloseTo(.06); expect(result.defaultTuning.softness).toBeCloseTo(.035); expect(result.defaultTuning.mask).toBe(0);
  expect(result.top[0]).toBeGreaterThan(220); expect(result.top[2]).toBe(0);
  expect(result.bottom[2]).toBeGreaterThan(220); expect(result.bottom[0]).toBe(0);
  expect(result.partialEqual).toBe(true);
  expect(result.partialRawEqual).toBe(true);
  expect(result.fullDrawFallback).toBe(true);
  expect(result.black).toEqual([0,0,0,255]);
  expect(result.line[1]).toBeGreaterThan(210); expect(result.neighbor[1]).toBeLessThan(12);
  expect(result.largeMask[0]).toBeLessThan(result.largePlain[0]);
  expect(result.largePlain[0]-result.largeMask[0]).toBeLessThan(6);
  expect(result.unchanged).toBe(true); expect(result.stable).toBe(true); expect(result.smallMaskEqual).toBe(true);
  expect(Number(result.futureAspect)).toBeCloseTo(1.6);
});

// Compare production raster bytes with the authoritative Wasm image. Existing
// CRT/GDC tests cover exact Japanese glyph, palette, PEGC and blink bytes.
test('production raster bytes remain exact with CRT enabled',async({page})=>{
  await ready(page); await page.locator('#presentation').selectOption('soft');
  await page.locator('#pause').click();
  const expected = await inspect(page,async()=>{
    const ptr=pc98.core.frame();
    return [...new Uint8Array(await crypto.subtle.digest('SHA-256',new Uint8Array(pc98.core.memory.buffer,ptr,640*400*4)))];
  });
  await expect.poll(()=>rawHash(page)).toEqual(expected);
  expect((await stats(page)).presenterStats.active).toBe(true);
});

test('high DPI output is bounded and resizes without applying aspect correction twice',async({browser},info)=>{
  const page=await browser.newPage({baseURL:info.project.use.baseURL,deviceScaleFactor:3,viewport:{width:1500,height:1100}});
  try {
    await ready(page); await page.locator('#presentation').selectOption('strong');
    await page.locator('#phosphor-mask').check();
    for (const viewport of [{width:1500,height:1100},{width:701,height:900},{width:480,height:800}]) {
      await page.setViewportSize(viewport);
      await expect.poll(async()=>{
        const s=(await stats(page)).presenterStats;
        const box=await page.locator('#screen').boundingBox();
        return Math.abs(s.width-Math.round(box.width*2))<2 && Math.abs(s.height-Math.round(box.height*2))<2;
      }).toBe(true);
      const s=(await stats(page)).presenterStats;
      expect(s.width).toBeLessThanOrEqual(2560); expect(s.height).toBeLessThanOrEqual(1920);
      expect(await page.locator('#screen').evaluate(c=>c.clientWidth/c.clientHeight)).toBeCloseTo(4/3,2);
      expect(s.active).toBe(true);
    }
  } finally {await page.close();}
});
