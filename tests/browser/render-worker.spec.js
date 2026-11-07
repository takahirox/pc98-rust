import {test,expect,inspect} from './helpers/runtime.js';

const stats=page=>page.evaluate(()=>pc98.stats());
const renderer=page=>page.workers().find(w=>w.url().includes('render-worker.js'));
async function screenHash(page) {
  return page.locator('#screen').evaluate(async canvas=>Array.from(new Uint8Array(
    await crypto.subtle.digest('SHA-256',canvas.getContext('2d').getImageData(0,0,640,400).data))));
}

test('render Worker rasterizes full coherent snapshots while inline rendering stays unused',async({page})=>{
  await page.clock.pauseAt(new Date());
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#pause').click();
  for(const profile of ['0','1','0','1']) {
    await page.locator('#profile').selectOption(profile);
    await expect(page.locator('#profile')).toBeEnabled();
    // Frame-memory growth occurs in the authoritative instance before capture.
    const expected=await inspect(page,async()=>{
      const core=pc98.core;
      core.memory.grow(1);
      const pointer=core.frame()>>>0;
      return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',
        new Uint8Array(core.memory.buffer,pointer,640*400*4))));
    });
    await page.clock.runFor(60);
    await expect.poll(async()=>{await page.clock.runFor(40);return screenHash(page);}).toEqual(expected);
  }
  const s=await stats(page);
  expect(s.videoMode).toBe('worker');
  expect(s.videoStats.inlineRasterMs).toBe(0);
  expect(s.renderStats.frames).toBeGreaterThan(0);
  expect(s.renderStats.rasterMs).toBeGreaterThan(0);
  expect(s.renderStats.copyMs).toBeGreaterThanOrEqual(0);
  // The manual DOM clock above is intentionally independent of Worker time.
});

test('a delayed old snapshot is discarded across profile/reset barriers and credits capture newest state',async({page})=>{
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#pause').click(); await expect(page.locator('#pause')).toHaveText('Resume');
  const worker=renderer(page);
  await worker.evaluate(()=>pc98Render.hold());
  await inspect(page,()=>pc98.core.memory.grow(1));
  await expect.poll(()=>stats(page).then(s=>s.frameInFlight)).toBe(true);
  const before=await stats(page);
  await page.locator('#profile').selectOption('0');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#reset').click(); await expect(page.locator('#profile')).toBeEnabled();
  expect((await stats(page)).videoEpoch).toBeGreaterThan(before.videoEpoch);
  expect(await worker.evaluate(()=>pc98Render.pending())).toBe(1);
  await worker.evaluate(()=>pc98Render.release());
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.stale)).toBeGreaterThan(before.presentationStats.stale);
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(before.presentationStats.frames);
  expect((await stats(page)).videoStats.coalesced).toBeGreaterThan(0);
  expect((await stats(page)).videoMode).toBe('worker');
});

test('switching to a paused profile replaces the screen after an invalidated snapshot returns',async({page})=>{
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('A:\\>');
  await page.locator('#pause').click(); await expect(page.locator('#profile')).toBeEnabled();
  await expect.poll(()=>stats(page).then(s=>s.frameInFlight)).toBe(false);
  const before=await screenHash(page), worker=renderer(page);
  await worker.evaluate(()=>pc98Render.hold());
  // With its credit available, set_profile captures a snapshot immediately.
  // The following running(false) command must request the final paused epoch.
  await page.locator('#profile').selectOption('0');
  await expect(page.locator('#profile')).toBeEnabled();
  const paused=await stats(page);
  expect(paused.running).toBe(false);
  await expect.poll(()=>worker.evaluate(()=>pc98Render.pending())).toBe(1);
  // Read the authoritative framebuffer directly: inspect() ends by presenting
  // a frame, which would mask a missing request in the production pause path.
  const emu=page.workers().find(w=>w.url().includes('emulator-worker.js'));
  const expected=await emu.evaluate(async()=>{
    const core=pc98.core, pointer=core.frame()>>>0;
    return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',
      new Uint8Array(core.memory.buffer,pointer,640*400*4))));
  });
  expect(expected).not.toEqual(before);
  await worker.evaluate(()=>pc98Render.release());
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.stale)).toBeGreaterThan(paused.presentationStats.stale);
  await expect.poll(()=>screenHash(page)).toEqual(expected);
  const after=await stats(page);
  expect(after.running).toBe(false);
  expect(after.instructions).toBe(paused.instructions);
  expect(after.videoEpoch).toBe(paused.videoEpoch);
  expect(after.videoMode).toBe('worker');
});

test('render startup, runtime and message failures recover through inline rendering without resetting the guest',async({page})=>{
  await page.route('**/render-worker.js',r=>r.fulfill({status:404,body:'missing'}));
  await page.goto('./'); await expect(page.locator('#game')).toBeEnabled();
  expect((await stats(page)).videoMode).toBe('inline');
  await page.locator('#hello').click();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
  await expect(page.locator('#error')).toBeHidden();
  await page.unrouteAll(); await page.reload(); await expect(page.locator('#game')).toBeEnabled();
  expect((await stats(page)).videoMode).toBe('worker');
  const before=await stats(page);
  await renderer(page).evaluate(()=>{setTimeout(()=>{throw Error('Intentional renderer failure');},0);});
  await expect.poll(()=>stats(page).then(s=>s.videoMode)).toBe('inline');
  const after=await stats(page);
  expect(after.generation).toBe(before.generation);
  expect(after.instructions).toBeGreaterThanOrEqual(before.instructions);
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(after.presentationStats.frames);
  await expect(page.locator('#error')).toBeHidden();
  await page.reload(); await expect(page.locator('#game')).toBeEnabled();
  await renderer(page).evaluate(()=>onmessage({data:{type:'frameAck',buffer:new ArrayBuffer(8),sequence:-1}}));
  await expect.poll(()=>stats(page).then(s=>s.videoMode)).toBe('inline');
  await expect(page.locator('#error')).toBeHidden();
  await page.reload(); await expect(page.locator('#game')).toBeEnabled();
  await page.workers().find(w=>w.url().includes('emulator-worker.js')).evaluate(()=>postMessage({event:'renderFault',error:'Intentional credit decode failure'}));
  await expect.poll(()=>stats(page).then(s=>s.videoMode)).toBe('inline');
  expect((await stats(page)).rendererFailure).toBe('Intentional credit decode failure');
});

test('Canvas context loss returns credits and restoration requests a fresh complete frame',async({page})=>{
  await page.goto('./'); await expect(page.locator('#game')).toBeEnabled();
  const before=await stats(page);
  await page.locator('#screen').evaluate(canvas=>canvas.dispatchEvent(new Event('contextlost',{cancelable:true})));
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.stale)).toBeGreaterThan(before.presentationStats.stale);
  await page.locator('#screen').evaluate(canvas=>canvas.dispatchEvent(new Event('contextrestored')));
  await expect.poll(()=>stats(page).then(s=>s.presentationStats.frames)).toBeGreaterThan(before.presentationStats.frames);
  expect((await stats(page)).videoEpoch).toBeGreaterThan(before.videoEpoch);
  expect((await stats(page)).rendererFailure).toBe(null);
});


test('emulation failure discards a queued image before a delayed rAF can present it',async({page})=>{
  await page.addInitScript(()=>{
    const callbacks=[];
    window.requestAnimationFrame=callback=>{callbacks.push(callback);return callbacks.length;};
    window.flushRender=()=>{for(const callback of callbacks.splice(0))callback(performance.now());};
  });
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('A:\\>');
  const before=await stats(page);
  // Request a full DOS-text image while presentation remains blocked.
  await page.locator('#screen').evaluate(canvas=>canvas.dispatchEvent(new Event('contextrestored')));
  await expect.poll(()=>stats(page).then(s=>s.renderStats.frames)).toBeGreaterThan(before.renderStats.frames);
  const emu=page.workers().find(w=>w.url().includes('emulator-worker.js'));
  await emu.evaluate(()=>{setTimeout(()=>{throw Error('Crash with pending image');},0);});
  await expect(page.locator('#status')).toContainText('reload to restart');
  await page.evaluate(()=>flushRender());
  expect(await page.locator('#screen').evaluate(canvas=>canvas.getContext('2d').getImageData(0,0,640,400).data.some((n,i)=>i%4!==3&&n!==0))).toBe(false);
});
