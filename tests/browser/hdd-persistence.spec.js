import { test, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { diskAction, openSettings, uploadMedia } from './helpers/player.js';

const shell = await readFile('fixtures/freedos/shell.hdi');
const header = Buffer.alloc(512);
header.write('T98HDDIMAGE.R0');
header.write('FreeDOS checkpoint test', 16);
header.writeUInt32LE(header.length, 0x110);
header.writeUInt32LE(shell.readUInt32LE(28), 0x114);
header.writeUInt16LE(shell.readUInt32LE(24), 0x118);
header.writeUInt16LE(shell.readUInt32LE(20), 0x11a);
header.writeUInt16LE(512, 0x11c);
const nhd = Buffer.concat([header, shell.subarray(shell.readUInt32LE(8))]);
const uploaded = {name:'local.NHD', mimeType:'application/octet-stream', buffer:nhd};

async function ready(page, name) {
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#media-name-2')).toHaveText(name);
}

async function dos(page, command, output) {
  await expect.poll(() => page.evaluate(() => pc98.text()), {timeout:20000}).toContain('A:\\>');
  await page.locator('#screen').focus();
  for (const char of command + '\r') {
    const key = char === '>' ? 'Period' : char === '.' ? 'Period' : char === ' ' ? 'Space' :
      char === '\r' ? 'Enter' : `Key${char.toUpperCase()}`;
    if (char === '>') await page.keyboard.down('Shift');
    await page.keyboard.down(key);
    await page.waitForTimeout(30);
    await page.keyboard.up(key);
    if (char === '>') await page.keyboard.up('Shift');
    await page.waitForTimeout(25);
  }
  if (output) await expect.poll(() => page.evaluate(() => pc98.text())).toContain(output);
}

async function save(page) {
  await diskAction(page, '2', 'save-hdd');
  await expect(page.locator('#status')).toContainText('HDD saved');
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#error')).toBeHidden();
}

async function exportImage(page) {
  const downloadEvent = page.waitForEvent('download');
  await diskAction(page, '2', 'export');
  const download = await downloadEvent;
  expect(download.suggestedFilename()).toBe(uploaded.name);
  return readFile(await download.path());
}

async function confirmAction(page, action, accept, message) {
  const seen = new Promise(resolve => page.once('dialog', async dialog => {
    const text = dialog.message();
    await (accept ? dialog.accept() : dialog.dismiss());
    resolve(text);
  }));
  await action();
  expect(await seen).toContain(message);
  await expect(page.locator('#profile')).toBeEnabled();
}

test('bundled FreeDOS boots a locally saved DOS copy across reload', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'The Pages project covers imported HDD startup.');
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await dos(page, 'copy probe.com local.com', 'probe.com =>> local.com\nA:\\>');
  await save(page);
  await expect(page.locator('#pause')).toHaveText('Pause');
  await page.locator('#profile').selectOption('0');
  await expect(page.locator('#media-name-2')).toContainText('FreeDOS-128MiB.hdi');
  await page.locator('#profile').selectOption('1');
  await page.reload();
  await ready(page, 'FreeDOS-128MiB.hdi');
  await dos(page, 'local', 'PC98 DOS PROBE OK');
  expect(await page.evaluate(() => performance.getEntriesByType('resource')
    .some(entry => entry.name.endsWith('/freedos/desktop.bin')))).toBe(false);
});

test('without Web Locks, FreeDOS boots only when the startup catalog allows it', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'The Web Locks fallback is independent of the serving path.');
  await page.addInitScript(() => Object.defineProperty(navigator, 'locks', {value:undefined}));
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await expect(page.locator('#error')).toBeHidden();
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('FreeDOS desktop ready');
  await page.locator('#drive-2').click();
  await expect(page.locator('#save-hdd')).toBeDisabled();
  await expect(page.locator('#hdd-save-status')).toContainText('HDD storage unavailable');

  await page.addInitScript(() => {
    indexedDB.open = () => { throw Error('Catalog unavailable'); };
  });
  await page.reload();
  await ready(page, 'Empty');
  await expect(page.locator('#error')).toContainText('HDD startup failed. Catalog unavailable');
});

test('imported NHD saves exact guest changes, restores a good checkpoint, and honors startup selection', async ({page}, info) => {
  test.skip(info.project.name !== 'pages', 'The local project covers bundled HDD startup and fault injection.');
  test.setTimeout(100000);
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await uploadMedia(page, uploaded, '2');
  await ready(page, uploaded.name);
  await dos(page, 'echo persisted>keep.txt');
  await dos(page, 'type keep.txt', '\npersisted');

  await page.locator('#pause').click();
  const savedImage = await exportImage(page);
  await save(page);
  await expect(page.locator('#pause')).toHaveText('Resume');
  const sum = bytes => createHash('sha256').update(bytes).digest('hex');
  expect(savedImage.length).toBe(nhd.length);
  expect(savedImage.subarray(0,512)).toEqual(header);
  expect(sum(savedImage)).not.toBe(sum(nhd));

  // Simulate checkpoint loss while the saved disk is still mounted and unchanged.
  await page.evaluate(async () => {
    const {HddStorage} = await import('./hdd-storage.js');
    const catalog = new HddStorage();
    await catalog.startup(async ({active}) => {
      const root = await navigator.storage.getDirectory();
      const dir = await root.getDirectoryHandle('pc98-ide-hdd-v1');
      await dir.removeEntry(`checkpoint-${active.slot}.bin`);
    });
  });
  await save(page);
  await page.reload();
  await ready(page, uploaded.name);
  await dos(page, 'type keep.txt', '\npersisted');
  expect(sum(await exportImage(page))).toBe(sum(savedImage));
  await dos(page, 'echo newer>keep.txt');
  await dos(page, 'type keep.txt', '\nnewer');
  await page.locator('#drive-2').click();
  await expect(page.locator('#hdd-save-status')).toContainText('Unsaved guest writes');
  await page.keyboard.press('Escape');
  await confirmAction(page, () => diskAction(page, '2', 'eject'), false, 'unsaved guest writes');
  await ready(page, uploaded.name);
  await save(page);

  // Damage a payload byte without changing the length or the container header.
  await page.evaluate(async () => {
    const storage = await import('./hdd-storage.js');
    const catalog = new storage.HddStorage();
    await catalog.startup(async ({active}) => {
      const root = await navigator.storage.getDirectory();
      const dir = await root.getDirectoryHandle('pc98-ide-hdd-v1');
      const handle = await dir.getFileHandle(`checkpoint-${active.slot}.bin`);
      const writer = await handle.createWritable({keepExistingData:true});
      const position = active.bytes - 1;
      const last = new Uint8Array(await (await handle.getFile()).slice(position).arrayBuffer());
      await writer.write({type:'write', position, data:new Uint8Array([last[0] ^ 0xff])});
      await writer.close();
    });
  });
  await page.reload();
  await ready(page, uploaded.name);
  await expect(page.locator('#error')).toContainText('previous checkpoint was restored');
  await dos(page, 'type keep.txt', '\npersisted');
  await save(page);

  // Cancel a clean replacement of a saved startup disk, then validate an
  // accepted replacement and the intentionally empty startup until it is saved.
  await confirmAction(page, () => uploadMedia(page, {...uploaded, name:'replacement.nhd'}, '2'), false, 'Replace the saved startup HDD');
  await ready(page, uploaded.name);
  await confirmAction(page, () => uploadMedia(page, {...uploaded, name:'replacement.nhd'}, '2'), true, 'Replace the saved startup HDD');
  await ready(page, 'replacement.nhd');
  await page.reload();
  await ready(page, 'Empty');

  await diskAction(page, '2', 'recover-hdd');
  await ready(page, uploaded.name);
  await dos(page, 'type keep.txt', '\npersisted');
  await confirmAction(page, () => diskAction(page, '2', 'eject'), true, 'Eject the saved startup HDD');
  await page.reload();
  await ready(page, 'Empty');
  await openSettings(page);
  await page.locator('#default-hdd').click();
  await ready(page, 'FreeDOS-128MiB.hdi');
  await page.reload();
  await ready(page, 'FreeDOS-128MiB.hdi');
});

async function storageTab(page, url) {
  await page.goto(url);
  await page.evaluate(async () => {
    const {HddStorage,chunkBytes,openCheckpoint} = await import('./hdd-storage.js');
    const storage = new HddStorage();
    await storage.startup(() => {});
    window.checkpointTest = {
      storage,
      async value(checkpoint = storage.current.active) {
        const image = await openCheckpoint(checkpoint);
        return new Uint8Array(await image.read(0))[0];
      },
      save(value, {stall = false, ...options} = {}) {
        return storage.save('test.hdi', chunkBytes + 512, (at,length) => {
          // The second producer request means the real OPFS Worker already
          // acknowledged the first write. Leave its file open until tab close.
          if (at && stall) {
            window.stagedHdd = true;
            return new Promise(() => {});
          }
          return new Uint8Array(length).fill(value).buffer;
        }, () => {}, options);
      },
    };
  });
}

test('saving after losing the latest checkpoint retains the older verified image', async ({page, baseURL}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  await storageTab(page, new URL('licenses.html', baseURL).href);
  const result = await page.evaluate(async () => {
    const {storage, save, value} = checkpointTest;
    const older = await save(1);
    const newest = await save(2);
    const root = await navigator.storage.getDirectory();
    const dir = await root.getDirectoryHandle('pc98-ide-hdd-v1');
    await dir.removeEntry(`checkpoint-${newest.slot}.bin`);
    const replacement = await save(3, {mountedCheckpoint:newest});

    const files = await Array.fromAsync(dir.keys());
    return {
      retained:storage.current.previous?.slot === older.slot,
      savedValue:await value(),
      retainedValue:await value(storage.current.previous),
      files:files.sort(),
      expectedFiles:[older, replacement].map(({slot}) => `checkpoint-${slot}.bin`).sort(),
    };
  });
  expect(result.retained).toBe(true);
  expect(result.savedValue).toBe(3);
  expect(result.retainedValue).toBe(1);
  expect(result.files).toEqual(result.expectedFiles);
});

test('staging interruption, catalog failure and concurrent writes preserve the durable generation', async ({page, context, baseURL}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  const url = new URL('licenses.html', baseURL).href;
  await storageTab(page, url);
  await page.evaluate(async () => { await checkpointTest.save(1); });
  const other = await context.newPage();
  await storageTab(other, url);
  await page.evaluate(() => { void checkpointTest.save(2, {stall:true}); });
  await expect.poll(() => page.evaluate(() => window.stagedHdd)).toBe(true);
  await expect(other.evaluate(() => checkpointTest.save(3))).rejects.toThrow('Another tab');
  await page.close();

  await storageTab(other, url);
  expect(await other.evaluate(() => checkpointTest.value())).toBe(1);
  const stale = await context.newPage();
  await storageTab(stale, url);
  const revision = await other.evaluate(() => checkpointTest.storage.current.revision);
  await expect(other.evaluate(async () => {
    const original = IDBObjectStore.prototype.put;
    IDBObjectStore.prototype.put = function (...args) {
      const request = original.apply(this, args);
      this.transaction.abort();
      return request;
    };
    try { await checkpointTest.save(3); }
    finally { IDBObjectStore.prototype.put = original; }
  })).rejects.toThrow('transaction aborted');
  await storageTab(other, url);
  expect(await other.evaluate(() => checkpointTest.storage.current.revision)).toBe(revision);
  expect(await other.evaluate(() => checkpointTest.value())).toBe(1);
  await other.evaluate(async () => { await checkpointTest.save(4); });
  await expect(stale.evaluate(() => checkpointTest.save(5))).rejects.toThrow('changed in another tab');
  await storageTab(other, url);
  expect(await other.evaluate(() => checkpointTest.value())).toBe(4);
  const files = await other.evaluate(async () => {
    const root = await navigator.storage.getDirectory();
    const dir = await root.getDirectoryHandle('pc98-ide-hdd-v1');
    return Array.fromAsync(dir.keys());
  });
  expect(files).toHaveLength(2);
});
