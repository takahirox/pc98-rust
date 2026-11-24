import { test, expect } from '@playwright/test';
import { diskAction, launchDemo, nextBoot, openSettings, uploadMedia } from './helpers/player.js';
import { mkdtemp, readFile, rm, truncate, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const hello = await readFile(new URL('../../web/media/hello98.tfd', import.meta.url));
const floppy = name => ({name, mimeType:'application/octet-stream', buffer:hello});
const stats = page => page.evaluate(() => pc98.stats());
const bootError = 'Boot failed: no readable PC-98 floppy or IDE IPL';

function floppyWithoutBootTrack() {
  // A valid D88 with one sector on side 1 and no track on side 0.
  const track = 0x2b0;
  const buffer = Buffer.alloc(track + 16 + 1024);
  buffer[0x1b] = 0x20;
  buffer.writeUInt32LE(buffer.length, 0x1c);
  buffer.writeUInt32LE(track, 0x24);
  buffer.set([0, 1, 1, 3, 1], track);
  buffer.writeUInt16LE(1024, track + 14);
  return {name:'no-boot-track.d88', mimeType:'application/octet-stream', buffer};
}

async function dropUnsupportedFile(page) {
  await page.locator('#screen').evaluate(canvas => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File(['unsupported'], 'unsupported.txt'));
    canvas.dispatchEvent(new DragEvent('drop', {bubbles:true, cancelable:true, dataTransfer}));
  });
}

async function expectNoticeClearOfScreen(page, initial, message = 'Unsupported media extension') {
  const screen = page.locator('#screen');
  const notice = page.locator('#error-notice');
  const alert = page.getByRole('alert');
  await expect(alert).toContainText(message);
  await expect(alert).toBeInViewport({ratio:1});
  await expect(notice).toBeInViewport({ratio:1});
  expect(await screen.boundingBox()).toEqual(initial);
  const box = await notice.boundingBox();
  expect(box.x >= initial.x + initial.width || box.x + box.width <= initial.x ||
    box.y >= initial.y + initial.height || box.y + box.height <= initial.y).toBe(true);
}

async function ready(page) {
  await page.goto('./');
  await expect.poll(() => page.evaluate(() => window.pc98?.text()), {timeout:20000}).toContain('A:\\>');
  await expect(page.locator('#crt-screen')).toBeVisible();
}

async function holdHddRead(page, size) {
  await page.evaluate(size => {
    const read = Blob.prototype.arrayBuffer;
    const wait = new Promise(resolve => { window.finishHddRead = resolve; });
    Blob.prototype.arrayBuffer = async function () {
      if (this.size === size) {
        Blob.prototype.arrayBuffer = read;
        await wait;
      }
      return read.call(this);
    };
  }, size);
}

async function dropHdd(page, target, name) {
  const hdi = await readFile(new URL('../../fixtures/ide_verify/ide_verify.hdi', import.meta.url));
  await holdHddRead(page, hdi.length);
  await target.evaluate((element, {name, base64}) => {
    const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([bytes], name, {type:'application/octet-stream'}));
    element.dispatchEvent(new DragEvent('drop', {bubbles:true, cancelable:true, dataTransfer}));
  }, {name, base64:hdi.toString('base64')});
}

const sizes = [
  {width:1280, height:720},
  {width:1440, height:900},
  {width:390, height:844, touch:true},
  {width:844, height:390, touch:true},
  {width:568, height:320, touch:true},
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
      expect(await page.locator('#crt-screen').boundingBox()).toEqual(screen);
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
      const alert = page.getByRole('alert');
      await expect(alert).toContainText('IDE HDD requires an HDI or NHD image');
      await expect(alert).toBeInViewport({ratio:1});
      if (width >= 740) {
        await expect(page.locator('#error-notice')).toBeInViewport({ratio:1});
        expect(await canvas.boundingBox()).toEqual(screen);
      }
      expect(await geometry()).toEqual(initial);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
      await page.getByRole('button', {name:'Dismiss error'}).click();
      await expect(alert).toBeHidden();
      expect(await geometry()).toEqual(initial);
    });
  });
}

test.describe('568 × 320 touch media picker', () => {
  test.use({viewport:{width:568,height:320}, hasTouch:true, isMobile:true});
  test('keeps the IDE file chooser action in view', async ({page}) => {
    await ready(page);
    const drive = page.getByRole('button', {name:'Change IDE HDD'});
    await drive.tap();
    const picker = page.getByRole('dialog', {name:'IDE HDD'});
    const choose = picker.getByRole('button', {name:'Choose local file for IDE HDD'});
    await expect(picker).toBeInViewport({ratio:1});
    await expect(choose).toBeInViewport({ratio:1});
    expect(await choose.evaluate(el => el.scrollWidth === el.clientWidth && el.clientWidth >= 150)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(568);
    await Promise.all([page.waitForEvent('filechooser'), choose.tap()]);
    await page.keyboard.press('Escape');
    await expect(picker).toBeHidden();
    await expect(drive).toBeFocused();
  });

  test('returns keyboard focus to Floppy 2 after dismissing a media error', async ({page}) => {
    await ready(page);
    const drive = page.getByRole('button', {name:'Insert Floppy 2'});
    for (let step = 0; step < 20 && !(await drive.evaluate(el => el === document.activeElement)); step++) {
      await page.keyboard.press('Tab');
    }
    await expect(drive).toBeFocused();
    await page.keyboard.press('Enter');
    const picker = page.getByRole('dialog', {name:'Floppy 2'});
    const choose = picker.getByRole('button', {name:'Choose local file for Floppy 2'});
    await expect(choose).toBeFocused();
    const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.keyboard.press('Enter')]);
    await chooser.setFiles({name:'incompatible.hdi', mimeType:'application/octet-stream', buffer:Buffer.alloc(1)});
    await expect(picker).toBeHidden();
    const alert = page.getByRole('alert');
    await expect(alert).toContainText('Hard disk images require IDE HDD');
    await expect(drive).toBeFocused();

    const dismiss = page.getByRole('button', {name:'Dismiss error'});
    for (let step = 0; step < 8 && !(await dismiss.evaluate(el => el === document.activeElement)); step++) {
      await page.keyboard.press('Shift+Tab');
    }
    await expect(dismiss).toBeFocused();
    await expect(dismiss).toBeInViewport({ratio:1});
    await page.keyboard.press('Enter');
    await expect(alert).toBeHidden();
    await expect(drive).toBeFocused();
    await expect(drive).toBeInViewport({ratio:1});
  });

  test('shows boot recovery at the current scroll position', async ({page}) => {
    await ready(page);
    await nextBoot(page, '0');
    await page.evaluate(() => window.scrollTo(0, 0));
    const normal = await page.locator('#screen').boundingBox();
    const reset = page.getByRole('button', {name:'Reset'});
    const dismiss = page.getByRole('button', {name:'Dismiss error'});
    await expect(reset).toBeInViewport({ratio:1});
    await reset.tap();
    await expectNoticeClearOfScreen(page, normal, bootError);
    await expect(dismiss).toBeInViewport({ratio:1});
    expect(await page.evaluate(() => scrollY)).toBe(0);
    await dismiss.tap();
    await expect(page.getByRole('alert')).toBeHidden();
    expect(await page.locator('#screen').boundingBox()).toEqual(normal);
  });

  test('keeps Keyboard and Settings tappable while a drop warning is visible', async ({page}) => {
    await ready(page);
    const screen = page.locator('#screen');
    const initial = await screen.boundingBox();
    const keyboard = page.getByRole('button', {name:'Keyboard', exact:true});
    const settings = page.getByRole('button', {name:'Settings'});
    const dismiss = page.getByRole('button', {name:'Dismiss error'});
    const tapInPlace = async control => {
      await expect(control).toBeInViewport({ratio:1});
      const {x, y, width, height} = await control.boundingBox();
      await page.touchscreen.tap(x + width / 2, y + height / 2);
    };
    await dropUnsupportedFile(page);
    await expectNoticeClearOfScreen(page, initial);
    await expect(dismiss).toBeInViewport({ratio:1});

    await tapInPlace(keyboard);
    await expect(keyboard).toHaveAttribute('aria-expanded', 'true');
    await tapInPlace(keyboard);
    await expect(keyboard).toHaveAttribute('aria-expanded', 'false');
    await expectNoticeClearOfScreen(page, initial);

    await tapInPlace(settings);
    const panel = page.getByRole('dialog', {name:'Settings'});
    await expect(panel).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expectNoticeClearOfScreen(page, initial);
    await dismiss.tap();
    await expect(page.getByRole('alert')).toBeHidden();
    expect(await screen.boundingBox()).toEqual(initial);
  });

  test('keeps HDD drop progress in view without moving the screen', async ({page}) => {
    await ready(page);
    const screen = page.locator('#screen');
    const initial = await screen.boundingBox();
    await dropHdd(page, screen, 'probe.hdi');
    const progress = page.getByRole('status').filter({hasText:'Importing HDD…'});
    await expect(progress).toBeInViewport({ratio:1});
    expect((await progress.boundingBox()).y).toBeGreaterThanOrEqual(initial.y + initial.height);
    expect(await screen.boundingBox()).toEqual(initial);
    expect(await page.evaluate(() => scrollY)).toBe(0);
    await page.evaluate(() => window.finishHddRead());
    await expect(progress).toBeHidden();
    await expect.poll(() => page.evaluate(() => pc98.text())).toContain('IDE VERIFY PASS');
  });
});

test.describe('320 × 700 touch settings', () => {
  test.use({viewport:{width:320,height:700}, hasTouch:true, isMobile:true});
  test('keeps the Next boot selector readable and reachable', async ({page}) => {
    await ready(page);
    const settings = page.getByRole('button', {name:'Settings'});
    await settings.tap();
    const panel = page.getByRole('dialog', {name:'Settings'});
    const boot = panel.getByRole('combobox', {name:'Next boot'});
    await expect(panel).toBeInViewport({ratio:1});
    await expect(boot).toBeInViewport({ratio:1});
    expect(await panel.evaluate(el => el.scrollWidth)).toBe(await panel.evaluate(el => el.clientWidth));
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(320);
    await boot.tap({trial:true});
    await boot.selectOption('0');
    await expect(boot).toHaveValue('0');
    await page.keyboard.press('Escape');
    await expect(panel).toBeHidden();
    await expect(settings).toBeFocused();
  });
});

test('FreeDOS auto-boot preserves keyboard focus on Display', async ({page}) => {
  const {promise: pending, resolve: release} = Promise.withResolvers();
  await page.route('**/freedos/desktop.bin', async route => {
    await pending;
    await route.continue();
  });
  await page.goto('./');
  await expect(page.getByRole('status').filter({hasText:'Loading FreeDOS HDD'})).toBeVisible();
  const display = page.getByRole('combobox', {name:'Display'});
  for (let step = 0; step < 10 && !(await display.evaluate(el => el === document.activeElement)); step++) {
    await page.keyboard.press('Tab');
  }
  await expect(display).toBeFocused();
  await expect(display).toHaveValue('sharp');

  release();
  await expect.poll(() => page.evaluate(() => pc98.text()), {timeout:20000}).toContain('A:\\>');
  await expect(display).toBeFocused();
  // Native select typeahead changes the preset even on macOS, where an arrow can open the menu.
  await page.keyboard.press('o');
  await expect(display).toHaveValue('off');
});

test('F12 captures standalone Shift and returns to host navigation', async ({page}) => {
  await ready(page);
  const screen = page.locator('#screen');
  const tabHeld = () => page.evaluate(async () => Boolean((await pc98.read(0x52b)) & 0x80));
  const shiftHeld = () => page.evaluate(async () => Boolean((await pc98.read(0x538)) & 0x01));
  await expect(screen).toBeFocused();
  await page.keyboard.down('Shift');
  const before = await stats(page);
  // Give the BIOS time to service an IRQ if the host modifier leaked.
  await expect.poll(async () => (await stats(page)).instructions).toBeGreaterThan(before.instructions + 10000);
  expect(await shiftHeld()).toBe(false);
  await page.keyboard.press('Tab');
  await expect(page.getByRole('link', {name:'GitHub'})).toBeFocused();
  await page.keyboard.up('Shift');
  expect(await tabHeld()).toBe(false);
  await page.keyboard.press('Tab');
  await expect(screen).toBeFocused();
  await page.keyboard.press('F12');
  await expect(screen).toBeFocused();
  for (const shift of ['ShiftLeft', 'ShiftRight']) {
    await page.keyboard.down(shift);
    await expect.poll(shiftHeld).toBe(true);
    await page.keyboard.up(shift);
    await expect.poll(shiftHeld).toBe(false);
  }
  await page.keyboard.press('F12');
  await expect(page.getByRole('button', {name:'Pause'})).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(screen).toBeFocused();
  await page.keyboard.down('Shift');
  await page.keyboard.press('ArrowRight');
  await expect.poll(shiftHeld).toBe(true);
  await page.keyboard.up('Shift');
  await expect.poll(shiftHeld).toBe(false);

  await page.keyboard.down('Tab');
  await expect.poll(tabHeld).toBe(true);
  await page.keyboard.down('Shift');
  await expect.poll(shiftHeld).toBe(true);
  await page.keyboard.up('Tab');
  await expect.poll(tabHeld).toBe(false);
  await page.keyboard.down('Tab');
  await expect(screen).toBeFocused();
  await expect.poll(tabHeld).toBe(true);

  await page.keyboard.press('F12');
  await expect(page.getByRole('button', {name:'Pause'})).toBeFocused();
  await expect.poll(tabHeld).toBe(false);
  await expect.poll(shiftHeld).toBe(false);
  await page.keyboard.up('Tab');
  await page.keyboard.up('Shift');

  await screen.focus();
  await page.keyboard.down('Shift');
  await screen.click();
  await expect.poll(shiftHeld).toBe(true);
  await page.keyboard.up('Shift');
  await expect.poll(shiftHeld).toBe(false);
});

test('standalone Ctrl, GRPH, XFER and NFER reach the guest after tabbing to the screen', async ({page}) => {
  await ready(page);
  const screen = page.locator('#screen');
  await expect(screen).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', {name:'Pause'})).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(screen).toBeFocused();
  for (const [code, scan] of [
    ['ControlLeft', 0x74], ['ControlRight', 0x74], ['AltLeft', 0x73], ['AltRight', 0x35], ['MetaLeft', 0x51],
  ]) {
    const held = () => page.evaluate(async scan => {
      const state = await pc98.read(0x52a + Math.floor(scan / 8));
      return Boolean(state & (1 << (scan % 8)));
    }, scan);
    await page.keyboard.down(code);
    await expect.poll(held, {message:`${code} make reaches the guest BIOS`}).toBe(true);
    await page.keyboard.up(code);
    await expect.poll(held, {message:`${code} break reaches the guest BIOS`}).toBe(false);
  }
  await page.keyboard.press('Shift+Tab');
  await expect(page.getByRole('link', {name:'GitHub'})).toBeFocused();
});

test('consecutive Space presses toggle playback and sound without moving focus', async ({page}) => {
  await ready(page);
  const pause = page.locator('#pause');
  const sound = page.locator('#sound');
  await page.keyboard.press('Tab');
  await expect(pause).toBeFocused();
  await page.keyboard.press('Space');
  await expect(pause).toBeEnabled();
  await expect(pause).toBeFocused();
  await expect(pause).toHaveText('Resume');
  expect((await stats(page)).running).toBe(false);
  await page.keyboard.press('Space');
  await expect(pause).toBeEnabled();
  await expect(pause).toBeFocused();
  await expect(pause).toHaveText('Pause');
  expect((await stats(page)).running).toBe(true);

  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  await expect(sound).toBeFocused();
  await page.keyboard.press('Space');
  await expect(sound).toHaveAttribute('aria-pressed', 'false');
  await expect(sound).toBeEnabled();
  await expect(sound).toBeFocused();
  await page.keyboard.press('Space');
  await expect(sound).toHaveAttribute('aria-pressed', 'true');
  await expect(sound).toBeEnabled();
  await expect(sound).toBeFocused();
});

test('keyboard changes keep focus on Next boot and Machine', async ({page}) => {
  await ready(page);
  const settings = page.getByRole('button', {name:'Settings'});
  for (let step = 0; step < 10 && !(await settings.evaluate(el => el === document.activeElement)); step++) {
    await page.keyboard.press('Tab');
  }
  await expect(settings).toBeFocused();
  await page.keyboard.press('Enter');
  await page.keyboard.press('Tab');
  const boot = page.getByRole('combobox', {name:'Next boot'});
  await expect(boot).toBeFocused();
  const before = await stats(page);
  // Native select typeahead also works in headless Chromium on macOS,
  // where an arrow key alone opens the options menu.
  await page.keyboard.press('f');
  await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · Empty');
  await expect(boot).toBeEnabled();
  await expect(boot).toBeFocused();
  expect((await stats(page)).generation).toBe(before.generation);

  await page.keyboard.press('Escape');
  await expect(settings).toBeFocused();
  await page.keyboard.press('Tab');
  const machine = page.getByRole('combobox', {name:'Machine'});
  await expect(machine).toBeFocused();
  await page.keyboard.press('p');
  await expect(page).toHaveTitle('pc98-rust · PC-9801VM');
  await expect(machine).toBeEnabled();
  await expect(machine).toBeFocused();
  expect((await stats(page)).generation).toBe(before.generation + 1);
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', {name:'Change IDE HDD'})).toBeFocused();
});

test.describe('320 × 700 touch media feedback', () => {
  test.use({viewport:{width:320,height:700}, hasTouch:true, isMobile:true});
  test('keeps HDD import progress and a recovery message in view', async ({page}) => {
    await ready(page);
    const hdi = await readFile(new URL('../../fixtures/ide_verify/ide_verify.hdi', import.meta.url));
    const drive = page.locator('#drive-2');
    const choose = page.getByRole('button', {name:'Choose local file for IDE HDD'});
    const tapToUpload = async file => {
      await drive.tap();
      const [chooser] = await Promise.all([page.waitForEvent('filechooser'), choose.tap()]);
      await chooser.setFiles(file);
    };
    const screen = page.locator('#screen');
    const initial = await screen.boundingBox();
    await expect(drive).toBeInViewport({ratio:1});
    await holdHddRead(page, hdi.length);
    await tapToUpload({name:'probe.hdi', mimeType:'application/octet-stream', buffer:hdi});
    const progress = page.getByText('Importing HDD…', {exact:true}).filter({visible:true});
    await expect(progress).toBeInViewport({ratio:1});
    expect(await screen.boundingBox()).toEqual(initial);
    for (const id of ['drive-2', 'drive-0', 'reset', 'pause', 'sound']) {
      await expect(page.locator('#' + id)).toBeDisabled();
    }
    await page.evaluate(() => window.finishHddRead());
    await expect(progress).toBeHidden();
    await expect(page.locator('#media-name-2')).toHaveText('probe.hdi');
    await expect.poll(() => page.evaluate(() => pc98.text())).toContain('IDE VERIFY PASS');

    await tapToUpload(floppy('incompatible.tfd'));
    const alert = page.getByRole('alert');
    await expect(alert).toContainText('Choose Insert media or a floppy drive for this image.');
    await expect(alert).toBeInViewport({ratio:1});
    expect(await screen.boundingBox()).toEqual(initial);
    await expect(page.locator('#media-name-2')).toHaveText('probe.hdi');
  });
});

test('keyboard navigation, picker dismissal, cancellation and selection preserve the running guest', async ({page}) => {
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
  await floppy1.focus();
  await page.keyboard.press('Tab');
  await expect(page.getByRole('button', {name:'Insert Floppy 2'})).toBeFocused();
  await page.keyboard.press('Enter');
  const floppy2Picker = page.getByRole('dialog', {name:'Floppy 2', exact:true});
  await expect(floppy2Picker.getByRole('button', {name:'Choose local file for Floppy 2'})).toBeFocused();
  const [chooser] = await Promise.all([page.waitForEvent('filechooser'), page.keyboard.press('Enter')]);
  await chooser.setFiles(floppy('second.tfd'));
  await expect(floppy2Picker).toBeHidden();
  await expect(page.locator('#media-name-1')).toHaveText('second.tfd');
  await expect(page.getByRole('button', {name:'Change Floppy 2'})).toBeFocused();

  // Keep the file read pending long enough to navigate to another control.
  await page.evaluate(() => {
    const read = File.prototype.arrayBuffer;
    const wait = new Promise(resolve => { window.finishFileRead = resolve; });
    File.prototype.arrayBuffer = async function () {
      if (this.name === 'replacement.tfd') await wait;
      return read.call(this);
    };
  });
  await uploadMedia(page, floppy('replacement.tfd'), '1');
  await expect(page.locator('#drive-1')).toBeDisabled();
  const settings = page.getByRole('button', {name:'Settings'});
  await settings.focus();
  await page.evaluate(() => window.finishFileRead());
  await expect(page.locator('#media-name-1')).toHaveText('replacement.tfd');
  await expect(settings).toBeFocused();

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

test('only the active media picker trigger is expanded and Escape returns focus', async ({page}) => {
  await page.setViewportSize({width:1280,height:720});
  await ready(page);
  const before = await stats(page);
  const floppy1 = page.getByRole('button', {name:'Insert Floppy 1'});
  const floppy2 = page.getByRole('button', {name:'Insert Floppy 2'});
  const session = await page.context().newCDPSession(page);
  const {root} = await session.send('DOM.getDocument');
  const {nodeIds} = await session.send('DOM.querySelectorAll', {nodeId:root.nodeId, selector:'[data-drive]'});
  const expanded = () => Promise.all(nodeIds.map(async nodeId => {
    const {nodes} = await session.send('Accessibility.getPartialAXTree', {nodeId,fetchRelatives:false});
    return nodes[0].properties?.find(property => property.name === 'expanded')?.value.value ?? false;
  }));
  await expect.poll(expanded).toEqual([false, false, false, false]);
  await floppy1.click();
  await expect(page.getByRole('button', {name:'Choose local file for Floppy 1'})).toBeFocused();
  await expect.poll(expanded).toEqual([false, true, false, false]);
  await floppy2.click();
  const picker = page.getByRole('dialog', {name:'Floppy 2', exact:true});
  await expect(picker.getByRole('button', {name:'Choose local file for Floppy 2'})).toBeFocused();
  await expect.poll(expanded).toEqual([false, false, true, false]);
  await page.keyboard.press('Escape');
  await expect(picker).toBeHidden();
  await expect(floppy2).toBeFocused();
  await expect.poll(expanded).toEqual([false, false, false, false]);
  await page.getByRole('button', {name:'Insert media', exact:true}).click();
  await expect.poll(expanded).toEqual([false, false, false, true]);
  await page.getByRole('heading', {name:'pc98-rust', exact:true}).click();
  await expect(page.getByRole('dialog', {name:'Insert media'})).toBeHidden();
  await expect.poll(expanded).toEqual([false, false, false, false]);
  await floppy1.click();
  await expect.poll(expanded).toEqual([false, true, false, false]);
  await floppy1.click();
  await expect.poll(expanded).toEqual([false, false, false, false]);
  await session.detach();
  expect((await stats(page)).generation).toBe(before.generation);
});

test('Settings records the floppy inserted before the paused VM first boots', async ({page}) => {
  await ready(page);
  await page.getByRole('button', {name:'Pause'}).click();
  await page.getByRole('combobox', {name:'Machine'}).selectOption('0');
  await expect(page.locator('#status')).toHaveText('No boot disk');
  const before = await stats(page);
  await uploadMedia(page, floppy('hello.tfd'), '0');
  await expect(page.locator('#media-name-0')).toHaveText('hello.tfd');
  await expect(page.locator('#status')).toHaveText('Paused');
  await diskAction(page, '0', 'eject');
  await expect(page.locator('#status')).toHaveText('No boot disk');
  await uploadMedia(page, floppy('hello.tfd'), '0');
  await expect(page.locator('#status')).toHaveText('Paused');
  expect((await stats(page)).running).toBe(false);
  expect((await stats(page)).generation).toBe(before.generation);
  await page.getByRole('button', {name:'Resume'}).click();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('Hello world!!');
  await openSettings(page);
  const attempt = page.locator('#active-boot');
  await expect(attempt).toHaveText('Booted: Floppy 1 · hello.tfd');
  await page.keyboard.press('Escape');

  await uploadMedia(page, floppy('replacement.tfd'), '0');
  await expect(page.locator('#media-name-0')).toHaveText('replacement.tfd');
  await openSettings(page);
  await expect(attempt).toHaveText('Booted: Floppy 1 · hello.tfd');
  await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · replacement.tfd');
  expect((await stats(page)).generation).toBe(before.generation);
});

test('Settings keeps the failed boot source separate from the next reset', async ({page}) => {
  await ready(page);
  await uploadMedia(page, floppyWithoutBootTrack(), '0');
  await expect(page.locator('#media-name-0')).toHaveText('no-boot-track.d88');
  await nextBoot(page, '0');
  await page.getByRole('button', {name:'Reset'}).click();
  await expect(page.getByRole('alert')).toContainText('Boot failed');
  expect((await stats(page)).error).toContain('no readable PC-98 floppy or IDE IPL');
  await page.getByRole('button', {name:'Dismiss error'}).click();
  await openSettings(page);
  const attempt = page.locator('#active-boot');
  const resetSource = page.locator('#boot-source');
  await expect(attempt).toHaveText('Boot failed: Floppy 1 · no-boot-track.d88');
  await expect(resetSource).toHaveText('Reset boots: Floppy 1 · no-boot-track.d88');
  await page.locator('#boot').selectOption('1');
  await expect(resetSource).toHaveText('Reset boots: IDE HDD · FreeDOS-128MiB.hdi');
  await expect(attempt).toHaveText('Boot failed: Floppy 1 · no-boot-track.d88');
  await page.keyboard.press('Escape');

  await uploadMedia(page, floppy('hello.tfd'), '0');
  await expect(page.locator('#media-name-0')).toHaveText('hello.tfd');
  await openSettings(page);
  await expect(attempt).toHaveText('Boot failed: Floppy 1 · no-boot-track.d88');
  await page.keyboard.press('Escape');
  await page.getByRole('button', {name:'Reset'}).click();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('A:\\>');
  await openSettings(page);
  await expect(attempt).toHaveText('Booted: IDE HDD · FreeDOS-128MiB.hdi');
});

test('Settings retains the firmware failure after a readable HDD IPL finds no bootable partition', async ({page}) => {
  await ready(page);
  const prefix = await readFile(new URL('../../web/freedos/desktop.bin', import.meta.url));
  const manifest = JSON.parse(await readFile(new URL('../../web/freedos/manifest.json', import.meta.url)));
  const sectorSize = prefix.readUInt32LE(16);
  const partitionTable = prefix.readUInt32LE(8) + sectorSize;
  prefix.fill(0, partitionTable, partitionTable + sectorSize);
  const directory = await mkdtemp(join(tmpdir(), 'pc98-boot-'));
  try {
    const path = join(directory, 'no-partition.hdi');
    await writeFile(path, prefix);
    await truncate(path, manifest.bytes);
    await uploadMedia(page, path, '2');
    await expect(page.locator('#media-name-2')).toHaveText('no-partition.hdi');
    await expect(page.getByRole('alert')).toHaveText(bootError);
  } finally {
    await rm(directory, {recursive:true, force:true});
  }
  const failed = await stats(page);
  expect(failed.hddReads).toBe(2);
  expect(failed.running).toBe(false);
  await page.getByRole('button', {name:'Dismiss error'}).click();
  await openSettings(page);
  const attempt = page.locator('#active-boot');
  await expect(attempt).toHaveText('Boot failed: IDE HDD · no-partition.hdi');
  await page.locator('#boot').selectOption('0');
  await expect(page.locator('#boot-source')).toHaveText('Reset boots: Floppy 1 · Empty');
  expect((await stats(page)).generation).toBe(failed.generation);
  await expect(attempt).toHaveText('Boot failed: IDE HDD · no-partition.hdi');
});

test.describe('844 × 390 landscape recovery', () => {
  test.use({viewport:{width:844,height:390}, hasTouch:true, isMobile:true});
  test('keeps the image clear and media actions tappable after a boot failure', async ({page}) => {
    await ready(page);
    await uploadMedia(page, floppyWithoutBootTrack(), '0');
    await nextBoot(page, '0');
    const normal = await page.locator('#screen').boundingBox();
    await page.getByRole('button', {name:'Reset'}).tap();
    await expectNoticeClearOfScreen(page, normal, bootError);
    for (const [label, title] of [['Insert media', 'Insert media'], ['Demos', 'Boot a demo']]) {
      const button = page.getByRole('button', {name:label, exact:true});
      await expect(button).toBeInViewport({ratio:1});
      expect(await button.evaluate(el => {
        const {x, y, width, height} = el.getBoundingClientRect();
        return document.elementFromPoint(x + width / 2, y + height / 2)?.closest('button') === el;
      })).toBe(true);
      await button.tap();
      const picker = page.getByRole('dialog', {name:title});
      await expect(picker).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(picker).toBeHidden();
    }
    await page.getByRole('button', {name:'Dismiss error'}).tap();
    await expect(page.getByRole('alert')).toBeHidden();
  });

  test('keeps the guest image clear in fallback fullscreen with the keyboard open', async ({page}) => {
    await page.addInitScript(() => {
      Element.prototype.requestFullscreen = () => Promise.reject(new Error('Fullscreen unavailable'));
    });
    await ready(page);
    await page.getByRole('button', {name:'Fullscreen', exact:true}).tap();
    const exit = page.getByRole('button', {name:'Exit fullscreen'});
    await expect(page.locator('#drop')).toHaveClass(/full-window/);
    await page.locator('#fullscreen-keyboard').tap();
    const panel = page.locator('#drop #keyboard-panel');
    await expect(panel).toBeVisible();
    const fullscreen = await page.locator('#screen').boundingBox();
    await dropUnsupportedFile(page);
    await expectNoticeClearOfScreen(page, fullscreen);
    await expect(exit).toBeInViewport({ratio:1});
    await expect(panel.getByRole('button', {name:'STOP', exact:true})).toBeInViewport({ratio:1});
    await exit.tap();
    await expect(exit).toBeHidden();
    await expect(page.getByRole('alert')).toBeInViewport({ratio:1});
  });
});

test('keyboard entry into fallback fullscreen closes Help', async ({page}) => {
  await page.addInitScript(() => {
    Element.prototype.requestFullscreen = () => Promise.reject(new Error('Fullscreen unavailable'));
  });
  await ready(page);
  const help = page.getByRole('button', {name:'Help', exact:true});
  const panel = page.locator('#help-panel');
  const fullscreen = page.getByRole('button', {name:'Fullscreen', exact:true});
  await help.press('Enter');
  await expect(panel).toBeVisible();
  for (let step = 0; step < 20 && !(await fullscreen.evaluate(el => el === document.activeElement)); step++) {
    await page.keyboard.press('Tab');
  }
  await expect(fullscreen).toBeFocused();
  await expect(panel).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.locator('#drop')).toHaveClass(/full-window/);
  await expect(panel).toBeHidden();
  await expect(page.getByRole('button', {name:'Exit fullscreen'})).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(fullscreen).toBeFocused();
});

async function enterFullscreen(page, mode) {
  await page.setViewportSize({width:844,height:390});
  if (mode === 'fallback') {
    await page.addInitScript(() => {
      Element.prototype.requestFullscreen = () => Promise.reject(new Error('Fullscreen unavailable'));
    });
  }
  await ready(page);
  await page.getByRole('button', {name:'Fullscreen', exact:true}).press('Enter');
  await expect.poll(() => page.evaluate(() => document.fullscreenElement?.id ?? null))
    .toBe(mode === 'native' ? 'drop' : null);
  await expect(page.getByRole('button', {name:'Exit fullscreen'})).toBeFocused();
}

for (const mode of ['native', 'fallback']) {
  test(`${mode} fullscreen shows HDD drop progress with the exit in view`, async ({page}) => {
    await enterFullscreen(page, mode);
    const display = page.locator('#drop');
    await dropHdd(page, display, 'fullscreen.hdi');

    const progress = display.getByRole('status').filter({hasText:'Importing HDD…'});
    const exit = page.getByRole('button', {name:'Exit fullscreen'});
    await expect(progress).toBeInViewport({ratio:1});
    await expect(exit).toBeInViewport({ratio:1});
    await page.evaluate(() => window.finishHddRead());
    await expect.poll(() => page.evaluate(() => pc98.text())).toContain('IDE VERIFY PASS');
    await expect(display.locator('#media-progress')).toBeHidden();
    await exit.click();
    await expect(exit).toBeHidden();
    await expect(page.locator('#media-name-2')).toHaveText('fullscreen.hdi');
  });

  test(`${mode} fullscreen keeps Worker recovery visible and the exit reachable`, async ({page}) => {
    await enterFullscreen(page, mode);
    const fullscreen = page.getByRole('button', {name:'Fullscreen', exact:true});
    const exit = page.getByRole('button', {name:'Exit fullscreen'});
    await page.locator('#drop').getByRole('button', {name:'Keyboard', exact:true}).click();
    await expect(page.locator('#drop #keyboard-panel')).toBeVisible();

    const worker = page.workers().find(worker => worker.url().includes('emulator-worker.js'));
    await worker.evaluate(() => { setTimeout(() => { throw Error('Intentional fullscreen Worker failure'); }, 0); });
    const alert = page.getByRole('alert');
    await expect(alert).toContainText('Intentional fullscreen Worker failure. Reload to restart.');
    await expect(alert).toBeInViewport({ratio:1});
    await expect(exit).toBeInViewport({ratio:1});
    const session = await page.context().newCDPSession(page);
    const {root} = await session.send('DOM.getDocument');
    const {nodeId} = await session.send('DOM.querySelector', {nodeId:root.nodeId, selector:'#error'});
    const {nodes} = await session.send('Accessibility.getPartialAXTree', {nodeId,fetchRelatives:false});
    expect(nodes[0]).toMatchObject({ignored:false,role:{value:'alert'}});
    await session.detach();

    await page.locator('#screen').focus();
    await page.keyboard.press('F12');
    await expect(exit).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(exit).toBeHidden();
    await expect(fullscreen).toBeFocused();
    await expect(alert).toBeVisible();
  });
}

for (const [width, height] of [[568, 320], [390, 844]]) {
  test.describe(`${width} × ${height} fullscreen touch notices`, () => {
    test.use({viewport:{width,height}, hasTouch:true, isMobile:true});
    for (const mode of ['native', 'fallback']) {
      test(`${mode} fullscreen keeps the guest image and keyboard stable across a warning`, async ({page}) => {
        if (mode === 'fallback') {
          await page.addInitScript(() => {
            Element.prototype.requestFullscreen = () => Promise.reject(new Error('Fullscreen unavailable'));
          });
        }
        await ready(page);
        await page.getByRole('button', {name:'Fullscreen', exact:true}).tap();
        const fullscreen = page.locator(mode === 'native' ? '#drop:fullscreen' : '#drop.full-window');
        await fullscreen.getByRole('button', {name:'Keyboard', exact:true}).tap();
        const panel = fullscreen.locator('#keyboard-panel');
        const screen = fullscreen.locator('#screen');
        const exit = fullscreen.getByRole('button', {name:'Exit fullscreen'});
        await expect(screen).toBeInViewport({ratio:1});
        await expect(panel).toBeInViewport({ratio:1});
        const initial = await screen.boundingBox();
        const keys = await panel.boundingBox();
        expect(initial.height).toBeGreaterThanOrEqual(150);
        expect(initial.width / initial.height).toBeCloseTo(4/3, 2);

        await dropUnsupportedFile(page);
        await expectNoticeClearOfScreen(page, initial);
        expect(await panel.boundingBox()).toEqual(keys);
        const scrollTop = () => panel.locator('#keyboard-scroll').evaluate(el => el.scrollTop);
        await panel.getByRole('button', {name:'Scroll keyboard down'}).tap();
        await expect.poll(scrollTop).toBeGreaterThan(0);
        await expect(exit).toBeInViewport({ratio:1});
        await exit.tap({trial:true});

        await fullscreen.getByRole('button', {name:'Dismiss error'}).tap();
        await expect(page.getByRole('alert')).toBeHidden();
        await expect(screen).toBeInViewport({ratio:1});
        expect(await screen.boundingBox()).toEqual(initial);
        expect(await panel.boundingBox()).toEqual(keys);
        await panel.getByRole('button', {name:'Scroll keyboard up'}).tap();
        await expect.poll(scrollTop).toBe(0);
        await exit.tap();
        await expect(fullscreen).toHaveCount(0);
      });
    }
  });
}

for (const [width, height] of [[390, 844], [568, 320], [844, 390]]) {
  test.describe(`${width} × ${height} normal touch keyboard`, () => {
    test.use({viewport:{width,height}, hasTouch:true, isMobile:true});
    test('scrolls and uses guest keys with the display in view', async ({page}) => {
      await ready(page);
      await launchDemo(page, 'game', {touch:true});
      await expect.poll(() => page.evaluate(() => pc98.text())).toContain('STAR TRAIL');
      await page.evaluate(() => window.scrollTo(0, 0));
      const before = await stats(page);
      const screen = page.locator('#screen');
      const keyboard = page.locator('#toggle-keyboard');
      await expect(keyboard).toBeInViewport({ratio:1});
      await keyboard.tap();
      await expect(keyboard).toHaveAttribute('aria-expanded', 'true');
      const panel = page.locator('#keyboard-home #keyboard-panel');
      await expect(panel.getByRole('button', {name:'STOP', exact:true})).toBeInViewport({ratio:0.99});
      await expect(screen).toBeInViewport({ratio:1});
      const display = await screen.boundingBox();
      const keys = await panel.boundingBox();
      expect(display.width / display.height).toBeCloseTo(4/3, 2);
      if (width < height) expect(display.y + display.height).toBeLessThanOrEqual(keys.y);
      else expect(display.x + display.width).toBeLessThanOrEqual(keys.x);

      const scroll = panel.getByRole('button', {name:'Scroll keyboard down'});
      await expect(scroll).toBeInViewport({ratio:1});
      const move = panel.getByRole('button', {name:'→', exact:true});
      const moveInView = () => move.evaluate(key => {
        const scroll = key.closest('#keyboard-scroll').getBoundingClientRect();
        const bounds = key.getBoundingClientRect();
        return bounds.top >= scroll.top && bounds.bottom <= scroll.bottom;
      });
      for (let tap = 0; tap < 12 && !await moveInView(); tap++) await scroll.tap();
      expect(await panel.locator('#keyboard-scroll').evaluate(el => el.scrollTop)).toBeGreaterThan(0);
      await expect(move).toBeInViewport({ratio:0.99});
      await expect(screen).toBeInViewport({ratio:1});
      await move.tap();
      await expect.poll(() => page.evaluate(() => pc98.read(0x21408))).toBe(2);
      await expect(screen).toBeInViewport({ratio:1});
      expect(await page.evaluate(() => window.scrollY)).toBe(0);
      await keyboard.tap();
      await expect(panel).toBeHidden();
      expect((await stats(page)).generation).toBe(before.generation);
    });
  });
}

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
