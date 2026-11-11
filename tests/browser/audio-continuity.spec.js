import {test, expect, inspect} from './helpers/runtime.js';
import fs from 'node:fs/promises';
import {workerProfile} from './helpers/worker-profile.js';

// Continuous screenshots and DOM snapshots compete with the real-time guest
// and AudioWorklet on CI. Keep API/network traces and their stats results;
// capture visual evidence only after a failure, outside the playback window.
test.use({trace:{mode:'retain-on-failure',screenshots:false,snapshots:false}});

test.afterEach(async ({browser,page}, info) => {
  if (info.status === info.expectedStatus && !process.env.PC98_AUDIO_PROFILE_ALWAYS) return;
  try {
    await info.attach('audio-state', {body:JSON.stringify(await stats(page)),contentType:'application/json'});
    await info.attach('audio-screen', {body:await page.screenshot({timeout:5000}),contentType:'image/png'});
  } catch (error) { console.log('Audio failure state unavailable:', error.message); }
  try {
    const profile = await workerProfile(browser);
    await fs.mkdir(info.outputDir, {recursive:true});
    await fs.writeFile(info.outputPath('emulator.cpuprofile'), JSON.stringify(profile));
    const nodes = new Map(profile.nodes.map(n => [n.id,n.callFrame.functionName]));
    const counts = new Map();
    for (const id of profile.samples ?? []) counts.set(nodes.get(id), (counts.get(nodes.get(id)) ?? 0)+1);
    console.log('Worker CPU profile', JSON.stringify([...counts].sort((a,b)=>b[1]-a[1]).slice(0,12)));
  } catch (error) { console.log('Worker CPU profile unavailable:', error.message); }
});

const stats = page => page.evaluate(()=>pc98.stats());
const picture = page => page.locator('#screen').evaluate(canvas=>{
  const pixels=canvas.getContext('2d').getImageData(8,100,32,1).data;
  return [...pixels]; // Outside text, sprites and the animated wait mark.
});
async function enter(page) {
  await page.locator('#screen').focus(); await page.keyboard.press('Enter');
}
async function advancing(page) {
  const before=await stats(page);
  await expect.poll(async()=> (await stats(page)).consumedSamples).toBeGreaterThan(before.consumedSamples+4800);
}

test('ADV98 production playback remains continuous through scene changes', async ({page},testInfo) => {
  test.setTimeout(90000);
  // Use the second serving-path run to cover a real 44.1 kHz AudioWorklet,
  // without multiplying every guest/browser/profile combination.
  if(testInfo.project.name==='pages') await page.addInitScript(()=>{
    const Context=window.AudioContext;
    window.AudioContext=class extends Context { constructor(options) { super({...options,sampleRate:44100}); } };
  });
  // Cover default JIT at 48 kHz and the explicit interpreter at the Pages root.
  await page.goto(testInfo.project.name==='local' ? './?diagnostics' : './?diagnostics&jit=0');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  await expect(page.locator('#file')).toBeEnabled();
  await page.locator('#file').setInputFiles({name:'music-scenes.hdm',mimeType:'application/octet-stream',buffer:await fs.readFile('fixtures/dos-apps/music-scenes.hdm')});
  await page.locator('#boot').selectOption('0');
  await page.locator('#reset').click(); await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#pause').click(); await expect(page.locator('#pause')).toHaveText('Resume');
  // Accelerate only silent DOS/compiler/title setup. Playback below uses the
  // production timer exclusively, with no inspection calls that step the core.
  for (let n=0;n<2;n++) await inspect(page,()=>{
    for(let i=0;i<90;i++) {pc98.advance(100000);pc98.core.audio_clear();}
  });
  expect((await stats(page)).fmKeyEdges).toBe(0);
  await page.locator('#sound').click(); await page.locator('#pause').click();
  await enter(page);
  await expect.poll(async()=> (await stats(page)).fmKeyEdges).toBeGreaterThan(16);
  await page.waitForTimeout(1500); // Finish the first bitmap/sprite and dialogue.
  const first=await picture(page), observations=[], fmPitches=new Set(), ssgPeriods=new Set();
  let previous=await stats(page);
  const start=previous;
  expect(start.deviceSampleRate).toBe(testInfo.project.name==='pages' ? 44100 : 48000);
  for(let i=0;i<16;i++) {
    if(i===7) {
      await enter(page);
      // The DOM event loop remains responsive during guest bitmap/DOS work.
      await page.evaluate(()=>document.body.dataset.audioProbe='responsive');
      await expect(page.locator('body')).toHaveAttribute('data-audio-probe','responsive');
    }
    // Sample inside the window as well: a one-second poll can repeatedly hit
    // the same point of this short melody and falsely report a stuck pitch.
    for(let probe=0;probe<3;probe++) {
      await page.waitForTimeout(257);
      const s=await stats(page); fmPitches.add(s.fmPitch); ssgPeriods.add(s.ssgPeriod);
    }
    await page.waitForTimeout(229);
    const current=await stats(page);
    const deviceFrames=current.scheduledSamples-previous.scheduledSamples;
    const rendered=deviceFrames*48000/current.deviceSampleRate;
    const missed=current.underflows-previous.underflows;
    const source=current.producedSamples-previous.producedSamples;
    const evidence=JSON.stringify({source,rendered,missed,queued:current.queuedSamples,
      outstanding:current.outstandingSamples,deviceSampleRate:current.deviceSampleRate});
    expect(current.audioEpoch).toBe(start.audioEpoch);
    expect(current.error).toBe('');
    expect(current.droppedSamples).toBe(0); expect(current.overflows).toBe(0);
    expect(current.sequenceErrors).toBe(0); expect(current.checksumErrors).toBe(0);
    expect(current.sentSamples).toBe(current.producedSamples);
    expect(current.queuedBlocks).toBeLessThanOrEqual(6);
    expect(current.outstandingSamples).toBeGreaterThanOrEqual(0);
    expect(current.outstandingSamples).toBeLessThan(3360); // Target 60 ms + one finite slice.
    expect(rendered,evidence).toBeGreaterThan(45000);
    // Permit isolated host scheduling noise (<0.5%), reject the baseline's
    // ~6% sustained silence. No persistent dropout may exceed 100 ms.
    expect(missed,evidence).toBeLessThan(deviceFrames*0.005+128);
    expect(current.maxUnderflowRun).toBeLessThan(current.deviceSampleRate/10);
    expect(current.audioEnergy).toBeGreaterThan(previous.audioEnergy+0.1);
    const guest=(current.cycles-previous.cycles)*48000/8000000;
    expect(Math.abs(source-guest)).toBeLessThan(2); // No guest-time/sample loss.
    expect(Math.abs(source-rendered)).toBeLessThan(3360);
    observations.push({source,rendered,missed,queued:current.queuedSamples,edges:current.fmKeyEdges-previous.fmKeyEdges,
      soundIrqs:current.soundIrqEdges-previous.soundIrqEdges,fmPitch:current.fmPitch,ssgPeriod:current.ssgPeriod});
    previous=current;
  }
  console.log('ADV98 continuity',JSON.stringify(observations));
  expect(await picture(page)).not.toEqual(first);
  expect(previous.reads).toBeGreaterThan(start.reads+100); // Actual DOS bitmap/sprite reads.
  expect(previous.fmKeyEdges-start.fmKeyEdges).toBeGreaterThan(300);
  expect(previous.fmKeyEdges-start.fmKeyEdges).toBeLessThan(600); // t120 score, no tempo catch-up.
  expect(previous.irqAcks-start.irqAcks).toBeGreaterThan(2000);
  expect(previous.soundIrqEdges-start.soundIrqEdges).toBeGreaterThan(500);
  expect(fmPitches.size).toBeGreaterThanOrEqual(3);
  expect(ssgPeriods.size).toBeGreaterThanOrEqual(3);
  expect(previous.checkedBlocks).toBeGreaterThan(1000);
  expect(previous.beepEdges).toBe(0);
  expect(previous.jit.mode).toBe(testInfo.project.name==='local' ? 'jit' : 'interpreter');
  if(testInfo.project.name==='local') expect(previous.jit.failures).toBe(0);

  // Exercise the same live guest through every stream discontinuity. Each
  // barrier discards old PCM, recovers all six credits, then starts a fresh
  // sample sequence without changing the guest music clock.
  for(const id of ['pause','sound']) {
    const before=await stats(page);
    await page.locator('#'+id).click(); await expect(page.locator('#profile')).toBeEnabled();
    const stopped=await stats(page);
    expect(stopped.audioEpoch).toBeGreaterThan(before.audioEpoch);
    expect(stopped.queuedSamples).toBe(0); expect(stopped.audioBuffers).toBe(6);
    await page.locator('#'+id).click(); await advancing(page);
  }
  await page.evaluate(()=>pc98.audio.suspend());
  expect((await stats(page)).queuedSamples).toBe(0);
  await page.evaluate(()=>pc98.audio.resume()); await advancing(page);
  await page.evaluate(()=>{
    Object.defineProperty(document,'hidden',{configurable:true,value:true});
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.locator('#pause')).toHaveText('Resume');
  const hidden=await stats(page);
  expect(hidden.queuedSamples).toBe(0); expect(hidden.audioBuffers).toBe(6);
  await page.evaluate(()=>{delete document.hidden;document.dispatchEvent(new Event('visibilitychange'));});
  await page.waitForTimeout(100);
  expect((await stats(page)).cycles).toBe(hidden.cycles);
  await page.locator('#pause').click(); await advancing(page);
  await page.locator('#reset').click(); await expect(page.locator('#profile')).toBeEnabled();
  const reset=await stats(page);
  expect(reset.audioEpoch).toBeGreaterThan(hidden.audioEpoch);
  expect(reset.fmKeyEdges).toBe(0);
  await page.waitForTimeout(150);
  const fresh=await stats(page);
  expect(fresh.audioEnergy-reset.audioEnergy).toBe(0); // No stuck song after reset.
  expect(fresh.sequenceErrors).toBe(0); expect(fresh.checksumErrors).toBe(0);
});
