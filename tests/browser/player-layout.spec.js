import { test, expect } from '@playwright/test';
import { launchDemo, nextBoot, openSettings, uploadMedia } from './helpers/player.js';
import { readFile } from 'node:fs/promises';

const hello = await readFile(new URL('../../web/media/hello98.tfd', import.meta.url));
const floppy = name => ({name, mimeType:'application/octet-stream', buffer:hello});
const stats = page => page.evaluate(() => pc98.stats());

async function ready(page) {
  await page.goto('./');
  await expect.poll(() => page.evaluate(() => window.pc98?.text()), {timeout:20000}).toContain('A:\\>');
  await expect(page.locator('#crt-screen')).toBeVisible();
}

const sizes = [
  {width:1280, height:720},
  {width:1440, height:900},
  {width:390, height:844, touch:true},
  {width:844, height:390, touch:true},
  {width:320, height:700, touch:true},
];

for (const {width, height, touch = false} of sizes) {
  test.describe(`${width} × ${height}${touch ? ' touch' : ''}`, () => {
    test.use({viewport:{width,height}, hasTouch:touch, isMobile:touch});
    test('keeps guest geometry stable and leaves playback reachable', async ({page}) => {
      await ready(page);
      const stage = page.locator('.screen-stage');
      const canvas = page.locator('#screen');
      const geometry = () => stage.evaluate(el => {
        const {width,height} = el.getBoundingClientRect();
        return {width,height};
      });
      const initial = await geometry();
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
      const screen = await canvas.boundingBox();
      expect(screen.width / screen.height).toBeCloseTo(4/3, 2);
      expect(screen.y).toBeGreaterThanOrEqual(0);
      expect(screen.y + screen.height).toBeLessThan(height);
      const toolbar = await page.getByRole('group', {name:'Playback controls'}).boundingBox();
      expect(toolbar.y).toBeGreaterThanOrEqual(screen.y + screen.height);
      expect(toolbar.y + toolbar.height).toBeLessThanOrEqual(height);
      const media = await page.getByRole('complementary', {name:'Machine and media'}).boundingBox();
      if (width >= 740) {
        expect(media.x).toBeGreaterThan(screen.x + screen.width);
        expect((await page.locator('#settings').boundingBox()).y).toBeLessThan(height);
      } else expect(media.y).toBeGreaterThanOrEqual(toolbar.y + toolbar.height);
      if (touch) {
        for (const id of ['pause','reset','sound','fullscreen','drive-0','toggle-keyboard']) {
          const control = await page.locator('#'+id).boundingBox();
          expect(control.height).toBeGreaterThanOrEqual(44);
          expect(control.width).toBeGreaterThanOrEqual(44);
        }
      }
      await expect(page.locator('#keyboard-panel')).toBeHidden();
      await page.getByRole('button', {name:'Help'}).click();
      const help = page.getByRole('dialog', {name:'Player help'});
      await expect(help.getByRole('link', {name:'Licenses & media credits'})).toBeVisible();
      expect(await geometry()).toEqual(initial);
      await page.keyboard.press('Escape');
      await expect(help).toBeHidden();
      const name = 'very-long-disk-filename-'.repeat(8) + '.tfd';
      await uploadMedia(page, floppy(name), '1');
      const mounted = page.locator('#media-name-1');
      await expect(mounted).toHaveAttribute('title', name);
      await expect(mounted).toHaveText(name);
      expect(await mounted.evaluate(el => el.scrollWidth > el.clientWidth)).toBe(true);
      expect(await geometry()).toEqual(initial);
      await uploadMedia(page, floppy('incompatible.tfd'), '2');
      await expect(page.getByRole('alert')).toContainText('IDE HDD requires an HDI or NHD image');
      expect(await geometry()).toEqual(initial);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
    });
  });
}

test('keyboard navigation, picker dismissal and chooser cancellation preserve the running guest', async ({page}) => {
  await ready(page);
  const screen = page.getByRole('img', {name:'PC-98 screen'});
  await expect(screen).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', {name:'Pause'})).toBeFocused();
  const before = await stats(page);

  const floppy1 = page.getByRole('button', {name:'Insert Floppy 1'});
  // Walk from playback to the actual picker trigger without touching the guest.
  for (let step = 0; step < 12 && !(await floppy1.evaluate(el => el === document.activeElement)); step++) {
    await page.keyboard.press('Tab');
  }
  await expect(floppy1).toBeFocused();
  await page.keyboard.press('Enter');
  const picker = page.getByRole('dialog', {name:'Floppy 1', exact:true});
  const choose = picker.getByRole('button', {name:'Choose local file for Floppy 1'});
  await expect(choose).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(picker).toBeHidden();
  await expect(floppy1).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(choose).toBeFocused();

  await page.locator('#file').evaluate(input => {
    input.addEventListener('cancel', event => { input.dataset.canceled = String(event.isTrusted); }, {once:true});
  });
  const session = await page.context().newCDPSession(page);
  await session.send('Page.setInterceptFileChooserDialog', {enabled:true,cancel:true});
  await page.keyboard.press('Enter');
  await expect(page.locator('#file')).toHaveAttribute('data-canceled','true');
  await session.send('Page.setInterceptFileChooserDialog', {enabled:false});
  await session.detach();
  await expect(choose).toBeFocused();
  await page.getByRole('heading', {name:'pc98-rust'}).click();
  await expect(picker).toBeHidden();
  await expect(page.locator('#media-name-0')).toHaveText('Empty');
  await expect(page.locator('#media-name-2')).toHaveText('FreeDOS-128MiB.hdi');
  const after = await stats(page);
  expect(after.running).toBe(true);
  expect(after.generation).toBe(before.generation);
  expect(after.audioEpoch).toBe(before.audioEpoch);
  expect(after.instructions).toBeGreaterThan(before.instructions);

  await uploadMedia(page, floppy('hello.tfd'), '0');
  await nextBoot(page, '0');
  await openSettings(page);
  await expect(page.locator('#active-boot')).toHaveText('Booted: IDE HDD · FreeDOS-128MiB.hdi');
  await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · hello.tfd');
  await page.keyboard.press('Escape');
  expect((await stats(page)).generation).toBe(before.generation);
  await page.getByRole('button', {name:'Reset'}).click();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('Hello world!!');
  await openSettings(page);
  await expect(page.locator('#active-boot')).toHaveText('Booted: Floppy 1 · hello.tfd');
});

test.describe('fullscreen touch keyboard', () => {
  test.use({viewport:{width:844,height:390}, hasTouch:true, isMobile:true});
  test('uses guest keys and exits fullscreen without resetting', async ({page}) => {
    await ready(page);
    await launchDemo(page, 'game', {touch:true});
    await expect.poll(() => page.evaluate(() => pc98.text())).toContain('STAR TRAIL');
    const before = await stats(page);
    await page.getByRole('button', {name:'Fullscreen', exact:true}).tap();
    const exit = page.getByRole('button', {name:'Exit fullscreen'});
    await expect(exit).toBeVisible();
    const keyboard = page.locator('#fullscreen-keyboard');
    await keyboard.tap();
    await expect(keyboard).toHaveAttribute('aria-expanded','true');
    await expect(page.locator('#drop #keyboard-panel')).toBeVisible();
    const screen = await page.locator('#screen').boundingBox();
    const panel = await page.locator('#keyboard-panel').boundingBox();
    expect(screen.y + screen.height).toBeLessThanOrEqual(panel.y);
    expect(screen.width / screen.height).toBeCloseTo(4/3,2);
    await page.getByRole('button', {name:'Scroll keyboard down'}).tap();
    expect(await page.locator('#keyboard-scroll').evaluate(el => el.scrollTop)).toBeGreaterThan(0);
    const move = page.locator('#keyboard-panel').getByRole('button', {name:'→', exact:true});
    await move.tap();
    await expect.poll(() => page.evaluate(() => pc98.read(0x21408))).toBe(2);
    await expect(exit).toBeInViewport({ratio:1});
    await exit.tap();
    await expect(exit).toBeHidden();
    await expect(page.locator('#keyboard-home #keyboard-panel')).toBeHidden();
    await expect(page.locator('#toggle-keyboard')).toHaveAttribute('aria-expanded','false');
    expect((await stats(page)).generation).toBe(before.generation);
  });
});
