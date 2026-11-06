import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: MOV CR WRITE from all GPRs, CR0/CR3 readback and guest fault repair before and after reset', async ({ page }) => {
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/mov_cr_write/mov_cr_write.hdm', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/mov-cr-write-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const expected = [...Buffer.from(manifest.result_hex, 'hex')];
  await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'mov_cr_write.hdm', mimeType: 'application/octet-stream', buffer: image });
  await inspect(page, () => pc98.core.set_boot_source(0));
  await page.locator('#reset').click();
  await expect(page.locator('#media-name')).toContainText('mov_cr_write.hdm');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    let slots = 0;
    let text = '';
    for (let i = 0; i < 20; i++) {
      slots += await inspect(page, () => { const n = pc98.core.step(100000); pc98.core.audio_clear(); return n; });
      expect(await inspect(page, () => pc98.stats().error)).toBe('');
      text = await inspect(page, () => pc98.text());
      if (text.includes('MOV CR WRITE PASS') || text.includes('MOV CR WRITE FAIL')) break;
    }
    expect(slots).toBeLessThanOrEqual(manifest.slot_budget);
    expect(text).toContain('MOV CR WRITE PASS');
    expect(text).not.toContain('FAIL');
    expect(await inspect(page, () => Array.from({ length: 128 }, (_, i) => pc98.read(0x8000 + i)))).toEqual(expected);
    expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
    await expect(page.locator('#error')).toBeHidden();
    expect(errors).toEqual([]);
  }
});

for (const name of ['pe_pg']) {
  test(`Ap: ${name} transition without page tables commits then shuts down, cold and after reset`, async ({ page }) => {
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption('1');
    expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const image = await fs.readFile(new URL(`../../fixtures/mov_cr_write/${name}.hdm`, import.meta.url));
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/mov-cr-write-manifest.json', import.meta.url)));
    const variant = manifest.transition_variants[name];
    expect(createHash('sha256').update(image).digest('hex')).toBe(variant.image_sha256);
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: `${name}.hdm`, mimeType: 'application/octet-stream', buffer: image });
    await inspect(page, () => pc98.core.set_boot_source(0));
    await page.locator('#reset').click();
    await expect(page.locator('#media-name')).toContainText(`${name}.hdm`);
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      await inspect(page, () => { pc98.core.step(10000); pc98.core.audio_clear(); });
      const error = await inspect(page, () => pc98.stats().error);
      expect(error).toContain('protected page fault delivery failed');
      expect(error).toContain('triple fault/shutdown');
      expect(await inspect(page, () => pc98.core.step(10000))).toBe(0);
      expect(await inspect(page, () => pc98.stats().error)).toBe(error);
      expect(await inspect(page, () => pc98.text())).not.toContain('PASS');
      expect(Buffer.from(await inspect(page, () => pc98.exportDisk())).equals(image)).toBe(true);
    }
  });
}
