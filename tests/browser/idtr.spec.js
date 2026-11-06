import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';

test('Ap: IDTR probe, relocated vectors, wrapping addresses and terminal shutdown before and after reset', async ({ page }) => {
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/idtr/idtr.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/idtr-manifest.json', import.meta.url)));
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'idtr.hdm', mimeType: 'application/octet-stream', buffer: image });
  await inspect(page, () => pc98.core.set_boot_source(0));
  await page.locator('#reset').click();
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    expect(await inspect(page, () => pc98.core.metric(16))).toBe(0);
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await inspect(page, () => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await inspect(page, () => pc98.stats().error)).toBe('');
      text = await inspect(page, () => pc98.text());
      if (await inspect(page, () => pc98.core.metric(16)) || text.includes('IDTR FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('IDTR PASS');
    expect(text).not.toContain('FAIL');
    expect(await inspect(page, () => Array.from({ length: 80 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(await inspect(page, () => Array.from({ length: 7 }, (_, i) => pc98.read(0xfff9 + i)))).toEqual([...Buffer.from(manifest.last_operand_hex, 'hex')]);
    expect(await inspect(page, () => pc98.core.metric(16))).toBe(1);
    const retired = await inspect(page, () => pc98.stats().instructions);
    expect(await inspect(page, () => pc98.core.step(100000))).toBe(100000);
    expect(await inspect(page, () => pc98.stats().instructions)).toBe(retired);
    expect(await inspect(page, () => pc98.core.metric(16))).toBe(1);
    expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
  }
});
