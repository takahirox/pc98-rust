import {test,expect,inspect} from './helpers/runtime.js';
import fs from 'node:fs/promises';

// These are real release-Wasm integration tests, using independent cleared
// guests already prepared and licensed by the repository.
test('Worker progresses through DOS graphics and PCM with rAF stopped and DOM events responsive',async({page}) => {
  test.setTimeout(60000);
  const errors=[]; page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(() => {
    WebAssembly.instantiate = () => { throw Error('Wasm must never execute on the UI thread'); };
    window.domEvents = 0;
    window.addEventListener('runtime-probe',() => { document.body.dataset.responsive=String(++window.domEvents); });
  });
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  expect(page.workers()).toHaveLength(1);
  expect(await page.evaluate(() => 'core' in pc98)).toBe(false);
  await page.locator('#sound').click();
  const buffer=await fs.readFile('fixtures/dos-apps/98videop.hdm');
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({name:'98videop.hdm',mimeType:'application/octet-stream',buffer});
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#boot').selectOption('0');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.evaluate(() => { window.requestAnimationFrame = () => 1; document.getElementById('reset').click(); });
  await expect(page.locator('#profile')).toBeEnabled();
  const transition = await inspect(page, () => pc98.transitionStart());
  await expect.poll(()=>page.evaluate(()=>pc98.stats().then(s=>s.pcmWrites)),{timeout:30000}).toBeGreaterThan(100);
  const before=(await page.evaluate(()=>pc98.stats())).instructions;
  for(let i=1;i<=4;i++) {
    await page.evaluate(() => window.dispatchEvent(new Event('runtime-probe')));
    await expect(page.locator('body')).toHaveAttribute('data-responsive',String(i));
    await page.waitForTimeout(25);
  }
  await expect.poll(()=>page.evaluate(()=>pc98.text()),{timeout:30000}).toContain('VIDEO RETURNED');
  const state=await page.evaluate(()=>pc98.stats());
  expect(state.instructions).toBeGreaterThan(before);
  expect(state.cpuHz).toBe(66000000);
  expect(state.cpuCycles).toBeGreaterThan(state.cycles);
  expect(state.completedInstructions).toBeGreaterThan(0);
  expect(state.completedInstructions).toBeLessThanOrEqual(state.instructions);
  expect(state.guestSeconds).toBe(state.cycles/8000000);
  const duration = await inspect(page, start => pc98.transitionDuration(start), transition);
  expect(duration.guestSeconds).toBeGreaterThan(0);
  expect(duration.wallMs).toBeGreaterThan(0);
  console.log('video decode/return duration', JSON.stringify(duration));
  expect(state.pcmWrites).toBeGreaterThan(100);
  expect(state.audioEnergy).toBeGreaterThan(0.01);
  expect(state.scheduledSamples).toBeGreaterThan(1000);
  expect(state.frameInFlight).toBe(true);
  expect(state.audioBuffers).toBeGreaterThanOrEqual(0); expect(state.audioBuffers).toBeLessThanOrEqual(6);
  expect(state.queuedBlocks).toBeLessThanOrEqual(6);
  await page.evaluate(()=>document.getElementById('pause').click()); await expect(page.locator('#profile')).toBeEnabled();
  const paused=await page.evaluate(()=>pc98.stats());
  expect(paused.queuedBlocks).toBe(0); expect(paused.audioBuffers).toBe(6);
  await page.waitForTimeout(50);
  expect((await page.evaluate(()=>pc98.stats())).instructions).toBe(paused.instructions);
  expect(errors).toEqual([]);
});

test('audio mute, pause, resume, reset and interrupted AudioContext flush the sink',async({page}) => {
  await page.goto('./'); await expect(page.locator('#game')).toBeEnabled();
  await page.locator('#hello').click();
  await expect.poll(()=>page.evaluate(()=>pc98.stats().then(s=>s.audioEnergy))).toBeGreaterThan(1);
  for (const id of ['sound','sound','pause','pause','reset']) {
    const before=await page.evaluate(()=>pc98.stats());
    await page.locator('#'+id).click(); await expect(page.locator('#profile')).toBeEnabled();
    const after=await page.evaluate(()=>pc98.stats());
    if (id === 'pause' && !before.running) expect(after.audioEpoch).toBe(before.audioEpoch);
    else expect(after.audioEpoch).toBeGreaterThan(before.audioEpoch);
    expect(after.queuedBlocks).toBeLessThanOrEqual(6);
    if (!after.running || after.audioGain === 0) expect(after.queuedBlocks).toBe(0);
  }
  await page.evaluate(()=>pc98.audio.suspend());
  expect((await page.evaluate(()=>pc98.stats())).queuedBlocks).toBe(0);
  await page.evaluate(()=>pc98.audio.resume());
  await page.locator('#pause').click(); await expect(page.locator('#profile')).toBeEnabled();
  const stopped=await page.evaluate(()=>pc98.stats());
  expect(stopped.queuedBlocks).toBe(0); expect(stopped.audioBuffers).toBe(6);
});

test('frame snapshots survive memory growth, reset and delayed presentation on VM and Ap',async({page}) => {
  await page.goto('./'); await expect(page.locator('#profile')).toBeEnabled();
  for (const profile of ['0','1']) {
    await page.locator('#profile').selectOption(profile);
    await page.locator('#hello').click();
    await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
    await inspect(page,() => pc98.core.input_buffer(8*1024*1024));
    await page.locator('#reset').click();
    await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('Hello world!!');
    await expect.poll(()=>page.locator('#screen').evaluate(canvas=>{
      const data=canvas.getContext('2d').getImageData(0,0,640,16).data;
      return data.some((n,i)=>i%4!==3 && n!==0);
    })).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
  }
});

test('Worker and AudioWorklet load failures are visible and recoverable',async({page}) => {
  await page.route('**/emulator-worker.js',r=>r.fulfill({status:404,body:'missing'}));
  await page.goto('./'); await expect(page.locator('#error')).toBeVisible();
  await expect(page.locator('#game')).toBeDisabled();
  await page.unrouteAll(); await page.reload(); await expect(page.locator('#game')).toBeEnabled();
  // Chromium's AudioWorklet fetches bypass Playwright routing. Exercise the
  // real browser loader with a missing local asset, then restore its URL.
  await page.evaluate(()=>{
    const load=AudioWorklet.prototype.addModule;
    AudioWorklet.prototype.addModule=function(url) { return load.call(this,new URL('missing-audio-worklet.js',url)); };
    window.restoreWorkletLoader=()=>{AudioWorklet.prototype.addModule=load;};
  });
  await page.locator('#sound').click(); await expect(page.locator('#error')).toBeVisible();
  await expect(page.locator('#sound')).toHaveText('Enable sound');
  await page.evaluate(()=>window.restoreWorkletLoader()); await page.locator('#sound').click(); await expect(page.locator('#sound')).toHaveText('Mute');
});

test('visibility, repeated reset/profile controls and runtime Worker failure cannot resurrect stale state',async({page}) => {
  await page.goto('./'); await expect(page.locator('#game')).toBeEnabled();
  await page.locator('#game').click();
  await expect.poll(()=>page.evaluate(()=>pc98.text())).toContain('STAR TRAIL');
  const before=await page.evaluate(()=>pc98.stats());
  await page.evaluate(()=>{const button=document.getElementById('reset');button.click();button.click();});
  await expect(page.locator('#profile')).toBeEnabled();
  expect((await page.evaluate(()=>pc98.stats())).generation).toBe(before.generation+1);
  await page.locator('#screen').focus(); await page.keyboard.down('ArrowRight');
  await page.evaluate(()=>{
    Object.defineProperty(document,'hidden',{configurable:true,value:true});
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('#pause')).toHaveText('Resume');
  const paused=await page.evaluate(()=>pc98.stats());
  expect(paused.running).toBe(false); expect(paused.queuedBlocks).toBe(0);
  await page.evaluate(()=>{delete document.hidden;document.dispatchEvent(new Event('visibilitychange'));});
  await page.waitForTimeout(40);
  expect((await page.evaluate(()=>pc98.stats())).instructions).toBe(paused.instructions);
  await page.keyboard.up('ArrowRight');
  await page.locator('#pause').click(); await expect(page.locator('#profile')).toBeEnabled();
  await page.evaluate(()=>{
    const profile=document.getElementById('profile');
    profile.value='0';profile.dispatchEvent(new Event('change'));
    profile.value='1';profile.dispatchEvent(new Event('change'));
  });
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await inspect(page,()=>pc98.core.metric(21))).toBe(0);
  await expect(page.locator('#profile')).toHaveValue('0');
  await page.locator('#profile').selectOption('1'); await expect(page.locator('#profile')).toBeEnabled();
  const worker=page.workers()[0];
  await worker.evaluate(()=>{setTimeout(()=>{throw Error('Intentional runtime Worker failure');},0);});
  await expect(page.locator('#error')).toContainText('Intentional runtime Worker failure');
  await expect(page.locator('#status')).toContainText('reload to restart');
  for (const id of ['game','reset','file','profile']) await expect(page.locator('#'+id)).toBeDisabled();
  await page.reload(); await expect(page.locator('#game')).toBeEnabled();
  await expect.poll(()=>page.evaluate(()=>pc98.text()),{timeout:20000}).toContain('A:\\>');
});
