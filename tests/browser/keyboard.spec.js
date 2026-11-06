import { test, expect, inspect } from './helpers/runtime.js';
import fs from 'node:fs/promises';

for (const [profile, name] of [['0', 'VM'], ['1', 'Ap']]) {
  test(`${name}: original DOS keymap control observes held Return and release in actual Wasm`, async ({ page }) => {
    // Match Issue #25's 5M boot + 1M hold + 1M release schedule using the
    // runtime fixture's explicit manual Worker scheduling.
    const time = new Date('2026-01-01T00:00:00Z');
    await page.clock.install({ time });
    await page.clock.pauseAt(time);
    await page.goto('./');
    await expect(page.locator('#profile')).toBeEnabled();
    await page.locator('#profile').selectOption(profile);
    const buffer = await fs.readFile(new URL('../../fixtures/freedos/keymap.hdm', import.meta.url));
    await expect(page.locator('#file')).toBeEnabled(); await page.locator('#file').setInputFiles({ name: 'keymap.hdm', mimeType: 'application/octet-stream', buffer });
    await inspect(page, () => pc98.core.set_boot_source(0));
    await page.locator('#reset').click();
    expect(await inspect(page, () => pc98.core.memory instanceof WebAssembly.Memory)).toBe(true);
    const advance = async slices => {
      const error = await inspect(page, count => {
        for (let i = 0; i < count; i++) {
          pc98.advance(100000);
          if (pc98.stats().error) break;
        }
        return pc98.stats().error;
      }, slices);
      expect(error).toBe('');
    };
    const bitmap = () => inspect(page, () => Array.from({ length: 16 }, (_, i) => pc98.read(0x52a + i)));
    await advance(50);
    expect(await inspect(page, () => pc98.text())).toContain('KEYMAP READY');
    expect(await bitmap()).toEqual(Array(16).fill(0));
    await page.locator('#screen').focus();
    await page.keyboard.down('Enter');
    // Input is queued; BIOS work RAM changes when the interrupt is serviced.
    expect(await bitmap()).toEqual(Array(16).fill(0));
    await advance(10);
    expect(await inspect(page, () => pc98.text())).toContain('KEYMAP DOWN PASS');
    const held = Array(16).fill(0);
    held[3] = 0x10;
    expect(await bitmap()).toEqual(held);
    await page.keyboard.up('Enter');
    expect(await bitmap()).toEqual(held);
    await advance(10);
    const text = await inspect(page, () => pc98.text());
    for (const expected of ['KEYMAP DOWN PASS', 'KEYMAP UP PASS', 'PROBE SHELL RETURN', 'A:\\>']) {
      expect(text).toContain(expected);
    }
    expect(text).not.toContain('KEYMAP BWA STALE');
    expect(await bitmap()).toEqual(Array(16).fill(0));
    await expect(page.locator('#error')).toBeHidden();
    await page.locator('#reset').click();
    await expect(page.locator('#profile')).toBeEnabled();
    expect(await bitmap()).toEqual(Array(16).fill(0));
  });
}
