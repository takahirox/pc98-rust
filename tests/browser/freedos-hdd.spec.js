import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';

test('Ap: cleared FreeDOS HDD kernel handoff and responsive shell cold/reset', async ({ page }) => {
  test.setTimeout(120000);
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  const time = new Date('2026-01-01T00:00:00Z');
  await page.clock.install({ time });
  await page.clock.pauseAt(time);
  await page.goto('./');
  await expect(page.locator('#profile')).toBeEnabled();
  await page.locator('#profile').selectOption('1');
  expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
  const image = await fs.readFile(new URL('../../fixtures/freedos/shell.hdi', import.meta.url));
  const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/freedos-hdd-manifest.json', import.meta.url)));
  expect(createHash('sha256').update(image).digest('hex')).toBe(manifest.image_sha256);
  const at = manifest.header_bytes + (manifest.partition_lba + 26) * 512;
  const kernel = image.subarray(at, at + manifest.files['KERNEL  SYS'].bytes);
  await page.locator('#drive').selectOption({ value: '2' });
  await page.locator('#file').setInputFiles({ name: 'shell.hdi', mimeType: 'application/octet-stream', buffer: image });
  await expect(page.locator('#media-name')).toHaveText('shell.hdi');
  for (let run = 0; run < 2; run++) {
    if (run) await page.locator('#reset').click();
    // Only step/read the actual Rust interpreter. Observe the unmodified GPL
    // loader's jump, before the kernel can overwrite its initialization image.
    const handoff = await page.evaluate(() => {
      for (let slots = 0; slots < 100000; slots++) {
        if (pc98.core.metric(17) === 0x60 && pc98.core.metric(18) === 0) {
          return {
            slots, text: pc98.text(), reads: pc98.stats().hddReads,
            bytes: Array.from({ length: 49938 }, (_, i) => pc98.read(0x600 + i)),
          };
        }
        pc98.core.step(1);
      }
      return null;
    });
    expect(handoff).not.toBeNull();
    expect(handoff.text).toContain('FreeDOS(98) FAT Kernel GO!');
    expect(handoff.reads).toBe(122);
    expect(Buffer.from(handoff.bytes).equals(kernel)).toBe(true);
    async function until(expected) {
      const text = await page.evaluate(({ expected, budget }) => {
        for (let slots = 0; slots < budget; slots += 10000) {
          const text = pc98.text();
          if (text.includes(expected) || pc98.stats().error) return text;
          pc98.core.step(10000);
          pc98.core.audio_clear();
        }
        return pc98.text();
      }, { expected, budget: manifest.slot_budget });
      expect(text).toContain(expected);
      expect((await page.evaluate(() => pc98.stats())).error).toBe('');
      return text;
    }
    await until('A:\\>');
    expect(await page.evaluate(() => pc98.text())).toContain('HDD SHELL READY');
    async function type(scans) {
      await page.evaluate(scans => {
        for (const scan of scans) {
          pc98.core.key(scan, 1); pc98.core.step(50000);
          pc98.core.key(scan, 0); pc98.core.step(10000);
        }
      }, scans);
    }
    await type([0x19, 0x13, 0x18, 0x2d, 0x12, 0x1c]);
    await until('PC98 DOS PROBE OK');
    await type([0x12, 0x2b, 0x22, 0x18, 0x34, 0x18, 0x24, 0x1c]);
    await until('\nok');
    await page.evaluate(() => pc98.core.step(100000));
    const text = await page.evaluate(() => pc98.text());
    expect(text.split('\n').some(line => line.trim() === 'ok')).toBe(true);
    expect(text.split('A:\\>').length - 1).toBeGreaterThanOrEqual(3);
    expect(await page.evaluate(() => pc98.core.metric(19))).toBe(0);
    expect(await page.evaluate(() => pc98.core.metric(20))).toBeGreaterThan(0);
    const stats = await page.evaluate(() => pc98.stats());
    expect(stats.instructions).toBeLessThanOrEqual(manifest.slot_budget);
    expect(stats.reads).toBe(0); expect(stats.hddReads).toBeGreaterThan(122);
    expect(stats.hddWrites).toBe(0);
    expect(Buffer.from(await page.evaluate(() => pc98.exportDisk(2))).equals(image)).toBe(true);
    expect(errors).toEqual([]);
    await expect(page.locator('#error')).toBeHidden();
    console.log(`FreeDOS HDD boot ${run}: handoff ${handoff.slots} slots; responsive PROBE/ECHO shell`);
  }
});
