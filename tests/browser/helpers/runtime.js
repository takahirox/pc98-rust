import { test as base, expect } from '@playwright/test';

// Deterministic probes opt into manual scheduling explicitly. Pausing the DOM
// clock alone must never control the production Worker scheduler.
export const test = base.extend({
  page: async ({page}, use) => {
    let manual = false;
    const pauseAt = page.clock.pauseAt.bind(page.clock);
    page.clock.pauseAt = async (...args) => { manual = true; return pauseAt(...args); };
    const goto = page.goto.bind(page);
    page.goto = (url, options) => goto(url === './' ? `./?diagnostics=1${manual ? '&manual=1' : ''}` : url, options);
    await use(page);
  },
});
export { expect };

// Inspect the actual production Worker, with no second emulator, DOM memory
// view, source-evaluation RPC or guest-result mocks. A barrier orders preceding
// UI commands; each probe has a finite work allowance scaled to the selected core clock.
export async function inspect(page, fn, arg) {
  await expect(page.locator('#profile')).toBeEnabled();
  await page.evaluate(() => pc98.stats());
  const worker = page.workers().find(worker => worker.url().includes('emulator-worker.js'));
  if (!worker) throw Error('Production emulation Worker missing');
  await worker.evaluate(() => pc98.beginInspection());
  try { return await worker.evaluate(fn,arg); }
  finally { await worker.evaluate(() => pc98.endInspection()); }
}

// Media replacement must preserve every guest/audio observation. Host render
// epochs, credits, timings and instruction throughput may change independently.
export async function mediaGuestState(page) {
  return inspect(page, () => {
    const {videoStats, videoEpoch, videoMode, frameInFlight, instructionThroughput, ...stats} = pc98.stats();
    return {text:pc98.text(), stats};
  });
}
