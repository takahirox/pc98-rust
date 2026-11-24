import { bootInputProbe, inputReady, inputState } from './helpers/player.js';
import { test, expect, inspect } from './helpers/runtime.js';

async function start(page, {manual = false} = {}) {
  await page.goto(manual ? './?diagnostics=1&manual=1' : './');
  await expect(page.locator('#profile')).toBeEnabled();
  await bootInputProbe(page);
  await expect.poll(() => manual
    ? inspect(page, () => {
      pc98.advance(100000);
      return pc98.text();
    })
    : page.evaluate(() => pc98.text())).toContain(inputReady);
  await page.evaluate(() => {
    document.addEventListener('pointerlockchange', () => {
      document.documentElement.dataset.lockOwner = document.pointerLockElement?.id ?? '';
    });
  });
  return page.locator('#screen');
}

function guest(page, microseconds = 0) {
  return inspect(page, ([base, advance]) => {
    if (advance) pc98.advance(advance);
    const word = offset => pc98.read(base + offset) | pc98.read(base + offset + 1) << 8;
    return {
      key:pc98.read(base + 1), keys:word(2),
      x:word(6) << 16 >> 16, y:word(8) << 16 >> 16,
      buttons:pc98.read(base + 10), edges:word(11),
    };
  }, [inputState, microseconds]);
}

const owner = page => page.evaluate(() => document.pointerLockElement?.id ?? null);
async function capture(page, screen, options) {
  await screen.click(options);
  await expect(page.locator('html')).toHaveAttribute('data-lock-owner', 'screen');
  expect(await owner(page)).toBe('screen');
}

test('the real raw canvas captures with or without CRT and the guest receives relative motion and button edges', async ({page}) => {
  await page.setViewportSize({width:1600, height:900});
  const screen = await start(page);
  await page.bringToFront();
  await screen.focus();
  await expect.poll(() => owner(page)).toBeNull();
  await page.locator('#screen').press('Tab');
  await expect(page.locator('#pause')).toBeFocused();

  for (const preset of ['off', 'sharp']) {
    await page.locator('#presentation').selectOption(preset);
    if (preset === 'sharp') await expect(page.locator('#crt-screen')).toBeVisible();
    await screen.evaluate(canvas => {
      canvas.trustedMotion = null;
      canvas.recordOutsideMotion = false;
      const filter = new AbortController();
      canvas.stopMotionFilter = () => filter.abort();
      canvas.addEventListener('mousemove', event => {
        if (!event.isTrusted) return;
        if (canvas.recordOutsideMotion && event.movementX > canvas.clientWidth) {
          canvas.recordOutsideMotion = false;
          canvas.trustedMotion = {x:event.movementX, y:event.movementY};
          return;
        }
        // Chromium can emit cursor recenter events during capture and CDP
        // movement. Isolate the deliberate outside move and synthetic deltas.
        event.stopImmediatePropagation();
      }, {capture:true, signal:filter.signal});
    });
    const before = await guest(page);
    await capture(page, screen, {position:{x:12,y:100}});
    await expect(screen).toBeFocused();
    expect(await guest(page)).toMatchObject({buttons:0, edges:before.edges});

    const box = await screen.boundingBox();
    const x = box.x + 12, y = box.y + 100;
    await page.mouse.move(x + 4, y);
    await screen.evaluate(canvas => { canvas.recordOutsideMotion = true; });
    await page.mouse.move(x + box.width + 70, y + 180);
    // A move beyond the canvas still delivers trusted relative motion to it.
    const trustedMotion = () => screen.evaluate(canvas => canvas.trustedMotion);
    await expect.poll(async () => (await trustedMotion())?.x ?? 0).toBeGreaterThan(box.width);
    const motion = await trustedMotion();
    await expect.poll(() => guest(page)).toMatchObject({x:before.x + motion.x, y:before.y + motion.y});
    // Check the signed counter's backlog for a deterministic pair of deltas.
    const baseline = await guest(page);
    const dx = Math.ceil(box.width) + 100, dy = -273;
    await screen.evaluate((canvas, [movementX, movementY]) => {
      canvas.dispatchEvent(new MouseEvent('mousemove', {movementX, movementY}));
    }, [dx, dy]);
    await expect.poll(() => guest(page)).toMatchObject({x:baseline.x + dx, y:baseline.y + dy});
    await screen.evaluate(canvas => canvas.stopMotionFilter());

    await page.mouse.down();
    await expect.poll(() => guest(page)).toMatchObject({buttons:0x80, edges:before.edges + 1});
    await page.mouse.down({button:'right'});
    await expect.poll(() => guest(page)).toMatchObject({buttons:0xa0, edges:before.edges + 2});
    await page.mouse.up();
    await expect.poll(() => guest(page)).toMatchObject({buttons:0x20, edges:before.edges + 3});
    await page.mouse.up({button:'right'});
    await expect.poll(() => guest(page)).toMatchObject({buttons:0, edges:before.edges + 4});

    await page.keyboard.down('Escape');
    await expect.poll(() => owner(page)).toBeNull();
    await page.keyboard.down('Escape'); // A held host unlock key can repeat after lock loss.
    await page.keyboard.up('Escape');
    expect((await guest(page)).keys).toBe(before.keys);
    const unlocked = await guest(page);
    await page.mouse.move(x, y);
    expect(await guest(page)).toMatchObject({x:unlocked.x, y:unlocked.y, buttons:0});
    await page.keyboard.down('Escape');
    await expect.poll(() => guest(page)).toMatchObject({key:0, keys:before.keys + 1});
    await page.keyboard.down('Escape');
    await page.keyboard.up('Escape');
    await expect.poll(() => guest(page)).toMatchObject({key:0, keys:before.keys + 2});
  }
  await page.getByRole('button', {name:'Help', exact:true}).click();
  await expect(page.getByRole('dialog', {name:'Player help'})).toBeVisible();
});

test('real lock suppresses buttons held before capture until they are released', async ({page}) => {
  const screen = await start(page, {manual:true});
  await page.bringToFront();
  // Keep Chromium's native context menu out of the pre-capture button chord.
  await screen.evaluate(canvas => canvas.addEventListener('contextmenu', event => event.preventDefault()));
  const box = await screen.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  const before = await guest(page);

  await page.mouse.down({button:'right'});
  await page.mouse.down({button:'left'});
  await expect(page.locator('html')).toHaveAttribute('data-lock-owner', 'screen');
  expect(await owner(page)).toBe('screen');
  await page.mouse.up({button:'left'});
  expect(await guest(page, 100000)).toMatchObject({buttons:0, edges:before.edges});
  await page.mouse.up({button:'right'});
  expect(await guest(page, 100000)).toMatchObject({buttons:0, edges:before.edges});

  await page.mouse.down({button:'right'});
  expect(await guest(page, 100000)).toMatchObject({buttons:0x20, edges:before.edges + 1});
  await page.mouse.up({button:'right'});
  expect(await guest(page, 100000)).toMatchObject({buttons:0, edges:before.edges + 2});
  await page.keyboard.press('Escape');
  await expect.poll(() => owner(page)).toBeNull();
});

test('real lock releases on F12, host controls, visibility, fullscreen exit, reset and Worker fault', async ({page}) => {
  const screen = await start(page);
  await page.bringToFront();
  await capture(page, screen);
  await page.mouse.down();
  await expect.poll(() => guest(page)).toMatchObject({buttons:0x80});
  await page.keyboard.down('ArrowRight');
  await expect.poll(() => inspect(page, () => pc98.read(0x531) & 0x10)).toBe(0x10);
  await page.keyboard.press('F12');
  await expect.poll(() => owner(page)).toBeNull();
  await expect(page.locator('#pause')).toBeFocused();
  await page.mouse.up();
  await page.keyboard.up('ArrowRight');
  await expect.poll(() => guest(page)).toMatchObject({buttons:0});
  await expect.poll(() => inspect(page, () => pc98.read(0x531) & 0x10)).toBe(0);

  for (const control of ['settings', 'toggle-keyboard', 'pause']) {
    await capture(page, screen);
    await page.locator('#' + control).evaluate(button => button.click());
    await expect.poll(() => owner(page)).toBeNull();
    if (control === 'settings') await page.keyboard.press('Escape');
    if (control === 'toggle-keyboard') await page.locator('#toggle-keyboard').click();
  }
  await expect(page.locator('#pause')).toHaveText('Resume');
  await screen.click();
  expect(await owner(page)).toBeNull();
  await page.locator('#pause').click();

  await capture(page, screen);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect.poll(() => owner(page)).toBeNull();

  await page.locator('#fullscreen').click();
  await expect(page.locator('#exit-fullscreen')).toBeVisible();
  await capture(page, screen);
  await page.locator('#exit-fullscreen').evaluate(button => button.click());
  await expect.poll(() => owner(page)).toBeNull();
  await expect(page.locator('#exit-fullscreen')).toBeHidden();

  await capture(page, screen);
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', {configurable:true, value:true});
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect.poll(() => owner(page)).toBeNull();
  await expect(page.locator('#pause')).toHaveText('Resume');
  await page.evaluate(() => { delete document.hidden; });
  expect(await owner(page)).toBeNull();
  await page.locator('#pause').click();

  await capture(page, screen);
  await page.locator('#reset').evaluate(button => button.click());
  await expect.poll(() => owner(page)).toBeNull();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain(inputReady);
  await capture(page, screen);
  await page.locator('#profile').selectOption('0');
  await expect.poll(() => owner(page)).toBeNull();
  await screen.evaluate(canvas => {
    canvas.requestPointerLock = () => { window.vmRequestedLock = true; };
  });
  await screen.click();
  expect(await page.evaluate(() => window.vmRequestedLock)).toBeUndefined();
  await page.locator('#profile').selectOption('1');
  await screen.evaluate(canvas => { delete canvas.requestPointerLock; });

  await capture(page, screen);
  const worker = page.workers().find(worker => worker.url().includes('emulator-worker.js'));
  await worker.evaluate(() => { setTimeout(() => { throw Error('Input test Worker fault'); }, 0); });
  await expect(page.locator('#status')).toHaveText('Worker stopped');
  await expect.poll(() => owner(page)).toBeNull();
});

for (const failure of ['legacy error event', 'rejected promise', 'unsupported']) {
  test(`${failure}: bounded mouse fallback works and stops at the canvas boundary`, async ({page}) => {
    const screen = await start(page);
    await screen.evaluate((canvas, mode) => {
      if (mode === 'unsupported') canvas.requestPointerLock = undefined;
      else if (mode === 'legacy error event') canvas.requestPointerLock = () => {
        setTimeout(() => {
          document.dispatchEvent(new Event('pointerlockerror'));
          window.captureDenied = true;
        }, 0);
      };
      else canvas.requestPointerLock = () => {
        window.captureDenied = true;
        return Promise.reject(new DOMException('Denied'));
      };
    }, failure);
    const box = await screen.boundingBox();
    const x = box.x + box.width / 2, y = box.y + box.height / 2;
    await page.mouse.click(x, y);
    if (failure !== 'unsupported') await page.waitForFunction(() => window.captureDenied);
    await expect.poll(() => owner(page)).toBeNull();
    await page.mouse.move(x - 1, y);
    await page.mouse.move(x + box.width / 8, y - box.height / 8);
    await expect.poll(async () => (await guest(page)).x).toBeCloseTo(80, -1);
    await page.mouse.down();
    await expect.poll(() => guest(page)).toMatchObject({buttons:0x80});
    const inside = await guest(page);
    expect(inside.x).toBeCloseTo(80, -1);
    expect(inside.y).toBeCloseTo(-50, -1);
    await page.mouse.move(box.x + box.width + 5, y);
    await expect.poll(() => guest(page)).toMatchObject({buttons:0, x:inside.x, y:inside.y});
    await page.mouse.up();
    await page.keyboard.press('Escape');
    await expect.poll(() => guest(page)).toMatchObject({key:0, keys:1});
    await expect(page.locator('#error')).toBeHidden();
  });
}

test('a short fallback tap reaches the guest when both edges arrive between samples', async ({page}) => {
  const screen = await start(page, {manual:true});
  await screen.evaluate(canvas => { canvas.requestPointerLock = undefined; });
  await screen.click();
  expect(await guest(page)).toMatchObject({buttons:0, edges:0});
  expect(await guest(page, 100000)).toMatchObject({buttons:0, edges:2});
});

test('a focused canvas retries a rejected lock without sending the acquisition button to the guest', async ({page}) => {
  const screen = await start(page);
  await screen.evaluate(canvas => {
    let first = true;
    canvas.requestPointerLock = () => {
      if (first) {
        first = false;
        return Promise.reject(new DOMException('Denied'));
      }
      Object.defineProperty(document, 'pointerLockElement', {configurable:true, value:canvas});
      document.dispatchEvent(new Event('pointerlockchange'));
    };
  });
  const position = {x:30, y:30};
  await screen.click({position});
  expect(await owner(page)).toBeNull();
  await expect(screen).toBeFocused();
  const before = await guest(page);
  await capture(page, screen, {position});
  expect(await guest(page)).toEqual(before);
});

test('a retry rejected after mouseup preserves fallback movement before the click', async ({page}) => {
  const screen = await start(page, {manual:true});
  await screen.evaluate(canvas => {
    canvas.requestPointerLock = () => {
      canvas.requestPointerLock = () => new Promise((_, reject) => {
        canvas.rejectLock = () => reject(new DOMException('Denied'));
      });
      return Promise.reject(new DOMException('Denied'));
    };
  });
  const position = {x:30, y:30};
  await screen.click({position});
  const before = await guest(page);
  const box = await screen.boundingBox();
  const x = box.x + position.x, y = box.y + position.y;
  await page.mouse.move(x, y);
  await page.mouse.move(x + box.width / 8, y);
  await page.mouse.down();
  await page.mouse.up();
  await screen.evaluate(canvas => canvas.rejectLock());
  // Both the motion and the replayed tap are in Wasm before the guest samples.
  expect(await guest(page)).toEqual(before);
  const after = await guest(page, 100000);
  expect(after).toMatchObject({buttons:0, edges:before.edges + 2, y:before.y});
  expect(after.x - before.x).toBeCloseTo(80, -1);
  expect(await owner(page)).toBeNull();
});

test('leaving the canvas before a delayed grant keeps the acquisition button suppressed', async ({page}) => {
  const screen = await start(page);
  await screen.evaluate(canvas => {
    canvas.requestPointerLock = () => { canvas.grantLock = () => {
      Object.defineProperty(document, 'pointerLockElement', {configurable:true, value:canvas});
      document.dispatchEvent(new Event('pointerlockchange'));
    }; };
  });
  const box = await screen.boundingBox();
  const x = box.x + box.width / 2, y = box.y + box.height / 2;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width + 5, y);
  await screen.evaluate(canvas => canvas.grantLock());
  expect(await owner(page)).toBe('screen');
  // The delayed grant is simulated, so return to the canvas for event delivery.
  await page.mouse.move(x, y);
  await page.mouse.down({button:'right'});
  await expect.poll(() => guest(page)).toMatchObject({buttons:0x20, edges:1});
  await page.mouse.up();
  expect(await guest(page)).toMatchObject({buttons:0x20, edges:1});
  await page.mouse.down();
  await expect.poll(() => guest(page)).toMatchObject({buttons:0xa0, edges:2});
  await page.mouse.up();
  await page.mouse.up({button:'right'});
  await expect.poll(() => guest(page)).toMatchObject({buttons:0});
});

test('Escape dismisses Help while a cancelled legacy request awaits its outcome', async ({page}) => {
  const screen = await start(page);
  await screen.evaluate(canvas => {
    canvas.requestPointerLock = () => { window.finishLock = () => {
      Object.defineProperty(document, 'pointerLockElement', {configurable:true, value:canvas});
      document.dispatchEvent(new Event('pointerlockchange'));
    }; };
    document.exitPointerLock = () => {
      delete document.pointerLockElement;
      document.dispatchEvent(new Event('pointerlockchange'));
    };
  });
  await screen.click();
  await expect.poll(() => page.evaluate(() => typeof window.finishLock)).toBe('function');
  const help = page.getByRole('dialog', {name:'Player help'});
  await page.getByRole('button', {name:'Help', exact:true}).click();
  await expect(help).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(help).toBeHidden();

  // Receiving the cancelled request's late success cannot capture a replacement guest.
  await page.locator('#profile').selectOption('0');
  await page.locator('#profile').selectOption('1');
  await page.evaluate(() => window.finishLock());
  expect(await owner(page)).toBeNull();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain(inputReady);
  expect(await guest(page)).toMatchObject({buttons:0, edges:0, x:0, y:0});
});

test.describe('touch', () => {
  test.use({hasTouch:true, isMobile:true, viewport:{width:390,height:844}});
  test('real touch sends bounded input and leaves the host controls accessible', async ({page}) => {
    const screen = await start(page);
    await screen.evaluate(canvas => {
      canvas.addEventListener('pointerdown', event => { canvas.dataset.testContact = event.pointerId; }, {once:true});
    });
    const cdp = await page.context().newCDPSession(page);
    const box = await screen.boundingBox();
    const x = Math.round(box.x + box.width / 2), y = Math.round(box.y + box.height / 2);
    const send = (type, touchPoints) => cdp.send('Input.dispatchTouchEvent', {type, touchPoints});
    await send('touchStart', [{x, y}]);
    await expect.poll(() => guest(page)).toMatchObject({buttons:0x80, edges:1});
    await send('touchMove', [{x:x + 40, y:y + 40}]);
    await expect.poll(async () => (await guest(page)).x).toBeGreaterThan(50);
    await send('touchEnd', []);
    await expect.poll(() => guest(page)).toMatchObject({buttons:0, edges:2});
    expect(await owner(page)).toBeNull();
    await expect.poll(() => screen.evaluate(canvas => canvas.hasPointerCapture(Number(canvas.dataset.testContact)))).toBe(false);
    await cdp.detach();
    await page.getByRole('button', {name:'Insert Floppy 2'}).click();
    await expect(page.getByRole('dialog', {name:'Floppy 2', exact:true})).toBeVisible();
  });
});
