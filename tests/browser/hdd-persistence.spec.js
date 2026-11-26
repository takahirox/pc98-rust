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
  const button = page.locator('#save-hdd');
  await button.press('Enter');
  await expect(page.locator('#status')).toContainText('HDD saved');
  await expect(page.locator('#hdd-save-status')).toHaveText('Saved');
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#error')).toBeHidden();
  await expect(button).toBeFocused();
}

async function exportImage(page) {
  const downloadEvent = page.waitForEvent('download');
  await diskAction(page, '2', 'export');
  const download = await downloadEvent;
  expect(download.suggestedFilename()).toBe(uploaded.name);
  return readFile(await download.path());
}

test('an HDD export that finishes while hidden resumes the guest on return', async ({page}) => {
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  const stats = () => page.evaluate(() => pc98.stats());
  await expect.poll(async () => (await stats()).running).toBe(true);

  const worker = page.workers().find(worker => worker.url().endsWith('/emulator-worker.js'));
  await worker.evaluate(() => {
    const handleMessage = self.onmessage;
    // Hold the first chunk request so the real snapshot stays open until the tab is hidden.
    self.onmessage = event => {
      if (event.data.type !== 'readChunk') return handleMessage(event);
      self.onmessage = handleMessage;
      self.finishExport = () => handleMessage(event);
    };
  });
  const downloading = page.waitForEvent('download');
  await diskAction(page, '2', 'export');
  await expect.poll(() => worker.evaluate(() => typeof self.finishExport)).toBe('function');
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', {configurable:true, value:true});
    document.dispatchEvent(new Event('visibilitychange'));
  });
  const paused = await stats();
  expect(paused).toMatchObject({running:false, transaction:'export'});

  await worker.evaluate(() => { self.finishExport(); delete self.finishExport; });
  const download = await downloading;
  expect(download.suggestedFilename()).toBe('FreeDOS-128MiB.hdi');
  expect(await download.failure()).toBeNull();
  await expect(page.locator('#profile')).toBeEnabled();
  expect(await stats()).toMatchObject({running:false, transaction:null, instructions:paused.instructions});

  await page.evaluate(() => {
    delete document.hidden;
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect.poll(async () => (await stats()).instructions).toBeGreaterThan(paused.instructions);
});

async function confirmAction(page, action, accept, message, ...additionalPrompts) {
  const prompts = [[accept, message], ...additionalPrompts];
  const seen = [];
  const handle = async dialog => {
    const [accepted] = prompts[seen.length] ?? [];
    seen.push(dialog.message());
    await (accepted ? dialog.accept() : dialog.dismiss());
  };
  page.on('dialog', handle);
  try {
    await action();
    await expect.poll(() => seen.length).toBe(prompts.length);
    await expect(page.locator('#profile')).toBeEnabled();
    prompts.forEach(([, expected], index) => expect(seen[index]).toContain(expected));
  } finally { page.off('dialog', handle); }
}

function readCatalog(page) {
  return page.evaluate(async () => {
    const {HddStorage} = await import('./hdd-storage.js');
    return new HddStorage().startup(() => {});
  });
}

test('bundled FreeDOS boots a locally saved DOS copy across reload', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'The Pages project covers imported HDD startup.');
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await dos(page, 'copy probe.com local.com', 'probe.com =>> local.com\nA:\\>');
  await expect(page.locator('#hdd-save-status')).toHaveText('Unsaved changes');
  await save(page);
  await expect(page.locator('#pause')).toHaveText('Pause');
  await page.locator('#profile').selectOption('0');
  await expect(page.locator('#media-name-2')).toContainText('FreeDOS-128MiB.hdi');
  await expect(page.locator('#save-hdd')).toBeDisabled();
  await expect(page.locator('#hdd-save-status')).toHaveText('Save requires Ap');
  await page.locator('#profile').selectOption('1');
  await expect(page.locator('#hdd-save-status')).toHaveText('Saved');
  await page.reload();
  await ready(page, 'FreeDOS-128MiB.hdi');
  await expect(page.locator('#hdd-save-status')).toHaveText('Saved');
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
  await expect(page.locator('#save-hdd')).toBeDisabled();
  await expect(page.locator('#hdd-save-status')).toContainText('HDD storage unavailable');

  await page.addInitScript(() => {
    indexedDB.open = () => { throw Error('Catalog unavailable'); };
  });
  await page.reload();
  await ready(page, 'Empty');
  await expect(page.locator('#error')).toContainText('HDD startup failed. Catalog unavailable');
});

test('unavailable OPFS allows confirmed session-only recovery with both checkpoints intact', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await uploadMedia(page, uploaded, '2');
  await ready(page, uploaded.name);
  await save(page);
  await dos(page, 'echo newer>keep.txt');
  await dos(page, 'type keep.txt', '\nnewer');
  await save(page);
  const before = await readCatalog(page);
  expect(before.previous.name).toBe(uploaded.name);

  await page.addInitScript(() => {
    if (location.search === '?opfs-unavailable') {
      navigator.storage.getDirectory = async () => {
        throw new DOMException('OPFS unavailable', 'NotReadableError');
      };
    }
  });
  await page.goto('./?opfs-unavailable');
  await ready(page, 'Empty');
  await expect(page.locator('#error')).toContainText('HDD startup failed. OPFS unavailable');

  await openSettings(page);
  await confirmAction(page, () => page.locator('#default-hdd').click(), true, 'Boot a fresh FreeDOS HDD on startup?',
    [true, 'for this session only']);
  await ready(page, 'FreeDOS-128MiB.hdi');
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('FreeDOS desktop ready');
  expect(await readCatalog(page)).toEqual(before);

  await page.reload();
  await ready(page, 'Empty');
  const backup = {...uploaded, name:'backup.nhd'};
  await confirmAction(page, () => uploadMedia(page, backup, '2'), true, 'Replace the saved startup HDD',
    [false, 'for this session only']);
  await ready(page, 'Empty');
  expect(await readCatalog(page)).toEqual(before);

  await confirmAction(page, () => uploadMedia(page, backup, '2'), true, 'Replace the saved startup HDD',
    [true, 'for this session only']);
  await ready(page, backup.name);
  await expect.poll(() => page.evaluate(() => pc98.text())).toContain('HDD SHELL READY');
  expect(await readCatalog(page)).toEqual(before);

  await page.goto('./');
  await ready(page, uploaded.name);
  await dos(page, 'type keep.txt', '\nnewer');
  await confirmAction(page, () => diskAction(page, '2', 'recover-hdd'), true, 'Make the previous HDD checkpoint the startup disk?');
  expect(await exportImage(page)).toEqual(nhd);
});

test('terminating the storage Worker during a write releases the live disk for export and retry', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await uploadMedia(page, uploaded, '2');
  await ready(page, uploaded.name);
  await save(page);
  const before = await readCatalog(page);
  await dos(page, 'echo newer>keep.txt');
  await dos(page, 'type keep.txt', '\nnewer');
  const liveImage = await exportImage(page);
  expect(liveImage).not.toEqual(nhd);

  await page.clock.install();
  await page.evaluate(() => {
    const NativeWorker = window.Worker;
    window.Worker = class extends NativeWorker {
      constructor(url, options) {
        super(url, options);
        if (!url.pathname?.endsWith('/hdd-storage-worker.js')) return;
        let replies = 0;
        this.addEventListener('message', event => {
          // Drop the first write acknowledgement after the real Worker staged bytes.
          if (++replies !== 2 || event.data.error) return;
          event.stopImmediatePropagation();
          this.terminate();
          window.Worker = NativeWorker;
          window.storageWorkerTerminated = true;
        });
      }
    };
  });
  await page.locator('#save-hdd').click();
  await expect.poll(() => page.evaluate(() => window.storageWorkerTerminated)).toBe(true);
  await expect(page.locator('#hdd-save-status')).toHaveText('Saving…');
  await expect(page.locator('#save-hdd')).toBeDisabled();
  await page.clock.fastForward('00:31');

  await expect(page.locator('#error')).toContainText('HDD save failed. HDD storage Worker timed out');
  await ready(page, uploaded.name);
  await expect(page.locator('#hdd-save-status')).toHaveText('Save failed · Retry or export');
  await expect(page.locator('#pause')).toHaveText('Pause');
  expect(await readCatalog(page)).toEqual(before);
  expect(await exportImage(page)).toEqual(liveImage);
  await expect(page.locator('#hdd-save-status')).toHaveText('Save failed · Retry or export');

  await save(page);
  expect((await readCatalog(page)).previous).toEqual(before.active);
  await page.reload();
  await ready(page, uploaded.name);
  expect(await exportImage(page)).toEqual(liveImage);
});

test('imported NHD saves exact guest changes, restores a good checkpoint, and honors startup selection', async ({page}, info) => {
  test.skip(info.project.name !== 'pages', 'The local project covers bundled HDD startup and fault injection.');
  test.setTimeout(100000);
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await uploadMedia(page, uploaded, '2');
  await ready(page, uploaded.name);
  await expect(page.locator('#hdd-save-status')).toHaveText('Not saved');
  await dos(page, 'echo persisted>keep.txt');
  await dos(page, 'type keep.txt', '\npersisted');

  await page.locator('#pause').click();
  const savedImage = await exportImage(page);
  await page.locator('#drive-1').click();
  await page.keyboard.press('Escape');
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
  await expect(page.locator('#hdd-save-status')).toHaveText('Unsaved changes');
  await confirmAction(page, () => diskAction(page, '2', 'eject'), false, 'unsaved guest writes');
  await ready(page, uploaded.name);
  await expect(page.locator('#hdd-save-status')).toHaveText('Unsaved changes');
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
  await expect(page.locator('#hdd-save-status')).toHaveText('Previous checkpoint');
  await dos(page, 'type keep.txt', '\npersisted');
  await save(page);

  // Cancel a clean replacement of a saved startup disk, then validate an
  // accepted replacement and the intentionally empty startup until it is saved.
  await confirmAction(page, () => uploadMedia(page, {...uploaded, name:'replacement.nhd'}, '2'), false, 'Replace the saved startup HDD');
  await ready(page, uploaded.name);
  await expect(page.locator('#hdd-save-status')).toHaveText('Saved');
  await confirmAction(page, () => uploadMedia(page, {...uploaded, name:'replacement.nhd'}, '2'), true, 'Replace the saved startup HDD');
  await ready(page, 'replacement.nhd');
  await expect(page.locator('#hdd-save-status')).toHaveText('Not saved');
  await page.reload();
  await ready(page, 'Empty');
  await expect(page.locator('#save-hdd')).toBeHidden();
  await expect(page.locator('#hdd-save-status')).toBeHidden();

  await diskAction(page, '2', 'recover-hdd');
  await ready(page, uploaded.name);
  await expect(page.locator('#hdd-save-status')).toHaveText('Saved');
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

test('saving a startup fallback preserves the newer checkpoint through a transient read error', async ({page}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  await page.goto('./');
  await ready(page, 'FreeDOS-128MiB.hdi');
  await uploadMedia(page, uploaded, '2');
  await ready(page, uploaded.name);
  await save(page);
  await dos(page, 'echo newer>keep.txt');
  await dos(page, 'type keep.txt', '\nnewer');
  await save(page);

  const before = await readCatalog(page);
  await page.addInitScript(name => {
    const original = FileSystemFileHandle.prototype.getFile;
    FileSystemFileHandle.prototype.getFile = function () {
      if (this.name === name) return Promise.reject(new DOMException('Temporary HDD read error', 'NotReadableError'));
      return original.call(this);
    };
    window.restoreCheckpointReads = () => { FileSystemFileHandle.prototype.getFile = original; };
  }, `checkpoint-${before.active.slot}.bin`);
  await page.reload();
  await ready(page, uploaded.name);
  await expect(page.locator('#error')).toContainText('previous checkpoint was restored');

  await page.locator('#save-hdd').click();
  await expect(page.locator('#profile')).toBeEnabled();
  await expect(page.locator('#error')).toContainText('HDD save failed. Temporary HDD read error');
  expect(await readCatalog(page)).toEqual(before);

  await page.evaluate(() => window.restoreCheckpointReads());
  await save(page);
  const after = await readCatalog(page);
  expect(after.active.digests).toEqual(before.previous.digests);
  expect(after.previous).toEqual(before.active);
  await confirmAction(page, () => diskAction(page, '2', 'recover-hdd'), true, 'Make the previous HDD checkpoint the startup disk?');
  await dos(page, 'type keep.txt', '\nnewer');
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
        const first = new Uint8Array(await image.read(0))[0];
        for (let at = chunkBytes; at < image.size; at += chunkBytes) await image.read(at);
        return first;
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

test('a temporary read error preserves both committed checkpoints for retry', async ({page, baseURL}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  const url = new URL('licenses.html', baseURL).href;
  await storageTab(page, url);
  const before = await page.evaluate(async () => {
    await checkpointTest.save(1);
    await checkpointTest.save(2);
    return checkpointTest.storage.current;
  });
  await expect(page.evaluate(async () => {
    const {storage, save} = checkpointTest;
    const names = [storage.current.active, storage.current.previous].map(({slot}) => `checkpoint-${slot}.bin`);
    const original = FileSystemFileHandle.prototype.getFile;
    FileSystemFileHandle.prototype.getFile = function () {
      if (names.includes(this.name)) return Promise.reject(new DOMException('Temporary HDD read error', 'NotReadableError'));
      return original.call(this);
    };
    try { await save(3); }
    finally { FileSystemFileHandle.prototype.getFile = original; }
  })).rejects.toThrow('Temporary HDD read error');

  await storageTab(page, url);
  expect(await page.evaluate(() => checkpointTest.storage.current)).toEqual(before);
  expect(await page.evaluate(async () => {
    const {storage, value} = checkpointTest;
    return [await value(), await value(storage.current.previous)];
  })).toEqual([2, 1]);
  await page.evaluate(async () => { await checkpointTest.save(3); });
  await storageTab(page, url);
  expect(await page.evaluate(async () => {
    const {storage, value} = checkpointTest;
    return [await value(), await value(storage.current.previous)];
  })).toEqual([3, 2]);
});

test('switching startup to FreeDOS after losing the mounted checkpoint retains recovery', async ({page, baseURL}, info) => {
  test.skip(info.project.name !== 'local', 'Storage transaction fault coverage runs once.');
  const url = new URL('licenses.html', baseURL).href;
  await storageTab(page, url);
  const older = await page.evaluate(async () => {
    const {storage, save} = checkpointTest;
    const older = await save(1);
    const mounted = await save(2);
    const root = await navigator.storage.getDirectory();
    const dir = await root.getDirectoryHandle('pc98-ide-hdd-v1');
    await dir.removeEntry(`checkpoint-${mounted.slot}.bin`);
    await storage.select('default');
    return older;
  });
  await storageTab(page, url);
  expect(await page.evaluate(() => checkpointTest.storage.current)).toMatchObject({active:'default', previous:older});
  expect(await page.evaluate(async () => {
    const {storage, save, value} = checkpointTest;
    await save(3);
    await storage.recover(value);
    return await value();
  })).toBe(1);
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
