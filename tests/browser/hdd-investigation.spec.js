// Original MIT independent controls for #113; no private media or host DOS.
import { test, expect } from '@playwright/test';
import fs from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { exportHdd } from './helpers/disk.js';

for (const name of ['fragmented12', 'fragmented16', 'wait', 'maskwait']) {
  test(`Ap: independent HDD ${name} handoff/input cold/reset`, async ({ page }) => {
    test.setTimeout(120000);
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption('1');
    expect(await page.evaluate(() => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const manifest = JSON.parse(await fs.readFile(new URL('../../fixtures/hdd-investigation-manifest.json', import.meta.url)));
    const image = await fs.readFile(new URL(`../../fixtures/freedos/${name}.hdi`, import.meta.url));
    const kernel = await fs.readFile(new URL('../../fixtures/freedos/hdd-kernel.sys', import.meta.url));
    const sha = b => createHash('sha256').update(b).digest('hex');
    expect(sha(image)).toBe(manifest.images[name].sha256);
    expect(sha(kernel)).toBe(manifest.images[name].files['KERNEL  SYS'].sha256);
    await page.locator('#drive').selectOption({ value: '2' });
    await page.locator('#file').setInputFiles({ name: `${name}.hdi`, mimeType: 'application/octet-stream', buffer: image });
    await expect(page.locator('#media-name')).toHaveText(`${name}.hdi`);
    async function step(slots) {
      await page.evaluate(slots => {
        for (let left = slots; left > 0; left -= 100000) pc98.core.step(Math.min(left, 100000));
        pc98.core.audio_clear();
      }, slots);
    }
    async function until(expected) {
      const text = await page.evaluate(expected => {
        for (let slots = 0; slots < 6000000; slots += 1000) {
          if (pc98.text().includes(expected) || pc98.stats().error || pc98.core.metric(0) >= 8000000) break;
          pc98.core.step(1000); pc98.core.audio_clear();
        }
        return pc98.text();
      }, expected);
      expect(text).toContain(expected);
      expect((await page.evaluate(() => pc98.stats())).error).toBe('');
    }
    async function type(keys) {
      for (const key of keys) {
        await page.evaluate(key => pc98.core.key(key, 1), key); await step(50000);
        await page.evaluate(key => pc98.core.key(key, 0), key); await step(10000);
      }
    }
    async function snapshot() {
      return page.evaluate(() => {
        let nulls = 0, literal = 0;
        for (let i = 0; i < 2000; i++) {
          const lo = pc98.read(0xa0000 + i * 2), hi = pc98.read(0xa0001 + i * 2);
          if (!lo && !hi) nulls++;
          if (lo === 0x3f && !hi) literal++;
        }
        return { nulls, literal, cs: pc98.core.metric(17), ip: pc98.core.metric(18),
          faults: pc98.core.metric(19), irq: pc98.core.metric(20), stats: pc98.stats() };
      });
    }
    for (let run = 0; run < 2; run++) {
      if (run) await page.locator('#reset').click();
      const loaded = await page.evaluate(length => {
        for (let i = 0; i < 100000; i++) {
          if (pc98.core.metric(17) === 0x60 && pc98.core.metric(18) === 0) {
            return Array.from({ length }, (_, i) => pc98.read(0x600 + i));
          }
          pc98.core.step(1);
        }
        return null;
      }, kernel.length);
      expect(loaded).not.toBeNull();
      expect(Buffer.from(loaded).equals(kernel)).toBe(true);
      expect(await page.evaluate(() => pc98.text())).toContain('FreeDOS(98) FAT Kernel GO!');
      const handoff = await snapshot();
      expect(handoff.nulls).toBe(1973);
      expect(handoff.literal).toBe(0);
      expect(handoff.faults).toBe(0);
      if (name === 'wait' || name === 'maskwait') {
        const base = await page.evaluate(() => {
          for (let slots = 0; slots < 6000000; slots++) {
            const base = pc98.core.metric(17) * 16 + 0x300;
            if (pc98.core.metric(18) === 0x200 &&
                Array.from({ length: 8 }, (_, i) => String.fromCharCode(pc98.read(base + i))).join('') === 'H98STATE') return base;
            pc98.core.step(1);
            if (slots % 10000 === 0) pc98.core.audio_clear();
          }
          return null;
        });
        expect(base).not.toBeNull();
        async function state() {
          return page.evaluate(base => Array.from({ length: 18 }, (_, i) => pc98.read(base + i)), base);
        }
        expect((await state()).slice(8, 10)).toEqual([1, name === 'maskwait' ? 1 : 0]);
        const ready = await snapshot();
        await step(100000);
        const waiting = await snapshot();
        expect(waiting.nulls).toBe(1986);
        expect(waiting.literal).toBe(0);
        expect(waiting.irq).toBeGreaterThan(ready.irq);
        expect((await state())[12]).toBeGreaterThan(0); // actual BIOS IRQ0 ticks
        await page.evaluate(() => pc98.core.key(0x1c, 1));
        await step(50000);
        if (name === 'maskwait') {
          expect((await state())[8]).toBe(1);
          expect((await state())[17]).toBe(0);
          expect((await snapshot()).nulls).toBe(1986);
          expect(await page.evaluate(() => pc98.read(0x528))).toBe(0); // no fabricated BIOS key
        }
        await until('POSTWAIT ENTER PASS');
        const result = await state();
        expect(result[8]).toBe(2);
        expect(result[17]).toBe(13);
        const ticks = result[12] + result[13] * 256;
        if (name === 'maskwait') expect(ticks).toBeGreaterThanOrEqual(64);
        else expect(ticks).toBeLessThan(64);
        await page.evaluate(() => pc98.core.key(0x1c, 0)); await step(10000);
      }
      await until('HDD SHELL READY');
      await until('A:\\>');
      const beforeInput = await snapshot();
      await type([0x19, 0x13, 0x18, 0x2d, 0x12, 0x1c]);
      await until('PC98 DOS PROBE OK');
      await type([0x12, 0x2b, 0x22, 0x18, 0x34, 0x18, 0x24, 0x1c]);
      const text = await page.evaluate(() => {
        for (let slots = 0; slots < 8000000 && pc98.core.metric(0) < 8000000; slots += 1000) {
          const text = pc98.text();
          if ((text.split('A:\\>').length - 1 >= 3 && text.split('\n').some(s => s.trim() === 'ok')) || pc98.stats().error) return text;
          pc98.core.step(1000); pc98.core.audio_clear();
        }
        return pc98.text();
      });
      expect(text.split('\n').some(s => s.trim() === 'ok')).toBe(true);
      expect(text.split('A:\\>').length - 1).toBeGreaterThanOrEqual(3);
      const final = await snapshot();
      expect(final.faults).toBe(0);
      expect(final.irq).toBeGreaterThan(beforeInput.irq);
      expect(final.stats.instructions).toBeLessThanOrEqual(manifest.slot_budget);
      expect(final.stats.error).toBe('');
      expect(final.stats.hddWrites).toBe(0);
      expect(final.stats.reads).toBe(0);
      expect((await exportHdd(page)).equals(image)).toBe(true);
      expect(errors).toEqual([]);
      console.log(`${name} run ${run}: handoff reads=${handoff.stats.hddReads}; shell reads=${beforeInput.stats.hddReads}; final instructions=${final.stats.instructions}, IRQs=${final.irq}`);
    }
  });
}
