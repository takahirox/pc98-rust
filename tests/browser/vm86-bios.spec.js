// Original MIT runnable monitor/guest, production Rust/Wasm. Issue #118.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: paged VM86 timer BIOS, work/buffer faults, IRQ callbacks and keyboard, cold/reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/vm86_bios/vm86_bios.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/vm86-bios-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  await page.locator('#file').setInputFiles({ name: 'vm86_bios.hdm', mimeType: 'application/octet-stream', buffer: image });
  await expect(page.locator('#media-name')).toContainText('vm86_bios.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    const injected = [false, false, false];
    let slots = 0;
    let text = '';
    while (slots < manifest.slot_budget) {
      const state = await page.evaluate(() => {
        const slots = pc98.core.step(10000);
        pc98.core.audio_clear();
        return { slots, phase: pc98.read(0x8000), waiting: pc98.read(0x8003), text: pc98.text(), error: pc98.stats().error };
      });
      slots += state.slots;
      expect(state.error).toBe('');
      expect(state.text).not.toContain('VM86 BIOS FAIL');
      text = state.text;
      if (text.includes('VM86 BIOS PASS')) break;
      if (state.phase < 3 && state.waiting && !injected[state.phase]) {
        await page.locator('#screen').focus();
        await page.keyboard.up('a');
        await page.keyboard.down('a');
        injected[state.phase] = true;
      }
    }
    await page.keyboard.up('a');
    expect(text).toContain('VM86 BIOS PASS');
    expect(injected).toEqual([true, true, true]);
    expect(await page.evaluate(() => Array.from({ length: 4 }, (_, i) => pc98.read(0x8000 + i))))
      .toEqual([...Buffer.from(manifest.result_hex, 'hex')]);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});
