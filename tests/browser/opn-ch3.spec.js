import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: guest channel-3 operator PCM pitches and keyed mode transitions before and after reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(message.text());
  });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/opn_ch3/opn_ch3.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/opn-ch3-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await page.locator('#file').setInputFiles({ name: 'opn_ch3.hdm', mimeType: 'application/octet-stream', buffer: image });
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    const result = await page.evaluate(budget => {
      const pcm = Array.from({ length: 16 }, () => []);
      let slots = 0;
      let text = '';
      let bounded = true;
      let maxQueue = 0;
      const keyEdges = Array.from({ length: 16 }, () => new Set());
      while (slots < budget) {
        const before = pc98.read(0x8001);
        slots += pc98.core.step(Math.min(1000, budget - slots));
        const after = pc98.read(0x8001);
        const count = pc98.core.audio_len();
        maxQueue = Math.max(maxQueue, count);
        const samples = new Float32Array(pc98.core.memory.buffer, pc98.core.audio_ptr(), count);
        bounded &&= samples.every(s => Number.isFinite(s) && Math.abs(s) <= 1);
        if (before === after && after >= 1 && after <= 16) {
          pcm[after - 1].push(...samples);
          keyEdges[after - 1].add(pc98.stats().fmKeyEdges);
        }
        pc98.core.audio_clear(); // Drain every 48 samples, far below queue cap.
        text = pc98.text();
        if (text.includes('OPN CH3 PASS') || text.includes('FAIL') || pc98.stats().error) break;
      }
      // Measure pitch from interpolated rising zero crossings of actual PCM.
      const stages = pcm.map((samples, stage) => {
        const crossings = [];
        for (let i = 0; i + 1 < samples.length; i++) {
          const a = samples[i], b = samples[i + 1];
          if (a <= 0 && b > 0) crossings.push(i - a / (b - a));
        }
        return { samples: samples.length, crossings: crossings.length,
          hz: 48000 * (crossings.length - 1) / (crossings.at(-1) - crossings[0]),
          keyEdges: [...keyEdges[stage]] };
      });
      return { slots, text, bounded, maxQueue, stages, stats: pc98.stats(),
        results: Array.from({ length: 64 }, (_, i) => pc98.read(0x8000 + i)) };
    }, manifest.slot_budget);
    expect(result.slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(result.text).toContain('OPN CH3 PASS');
    expect(result.text).not.toContain('FAIL');
    expect(result.stats.error).toBe('');
    expect(result.bounded).toBe(true);
    expect(result.maxQueue).toBeLessThanOrEqual(48);
    expect(result.stats.fmKeyEdges).toBe(8);
    expect(result.stats.writes).toBe(0);
    expect(result.stats.beepEdges).toBe(0);
    for (const stage of manifest.stages) {
      const actual = result.stages[stage.stage - 1];
      expect(actual.samples, `stage ${stage.stage}`).toBeGreaterThanOrEqual(3500);
      expect(actual.crossings).toBeGreaterThanOrEqual(3);
      const expected = (3993600 / 144) * (stage.fnum / 1048576) * 2 ** stage.block;
      expect(Math.abs(actual.hz - expected), `stage ${stage.stage}, operator ${stage.operator}, mode ${stage.mode}: ${actual.hz} vs ${expected} Hz`)
        .toBeLessThanOrEqual(Math.max(0.03, expected * 0.0002));
      expect(actual.keyEdges).toEqual([1 + 2 * (stage.operator - 1)]);
    }
    expect(result.results).toEqual([...Buffer.from(manifest.result_hex, 'hex')]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
    console.log(`OPN CH3 boot ${run}: ${result.slots} slots; samples ${result.stages.map(s => s.samples)}; Hz ${result.stages.map(s => s.hz.toFixed(3))}`);
  }
});
