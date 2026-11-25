export const chunkBytes = 1024 * 1024;
const workerReplyTimeoutMs = 30_000;
const storageName = 'pc98-ide-hdd-v1';
const slots = [0, 1, 2];
const fileName = slot => `checkpoint-${slot}.bin`;
const slotOf = image => slots.includes(image?.slot) ? image.slot : undefined;
const initial = {revision:null, active:'default', previous:null};

class InvalidCheckpoint extends Error {}

async function manifest(value) {
  const db = await new Promise((resolve,reject) => {
    const request = indexedDB.open(storageName, 1);
    request.onupgradeneeded = () => request.result.createObjectStore('startup');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  try {
    return await new Promise((resolve,reject) => {
      const tx = value
        ? db.transaction('startup', 'readwrite', {durability:'strict'})
        : db.transaction('startup', 'readonly');
      const store = tx.objectStore('startup');
      const request = value ? store.put(value, 'disk') : store.get('disk');
      tx.oncomplete = () => resolve(value ?? request.result ?? initial);
      tx.onabort = () => reject(tx.error ?? Error('HDD catalog transaction aborted'));
    });
  } finally { db.close(); }
}

async function readManifest() {
  const saved = await manifest();
  if (!saved || !Object.hasOwn(saved, 'active') || !Object.hasOwn(saved, 'previous') ||
      (saved !== initial && typeof saved.revision !== 'string')) {
    throw Error('Invalid HDD startup record');
  }
  return saved;
}

function locked(mode, operation) {
  if (!navigator.locks) throw Error('Browser HDD storage requires Web Locks');
  return navigator.locks.request(storageName, {mode, ifAvailable:mode === 'exclusive'}, lock => {
    if (!lock) throw Error('Another tab is using HDD storage. Retry when it finishes');
    return operation();
  });
}

async function directory(create = false) {
  return (await navigator.storage.getDirectory()).getDirectoryHandle(storageName, {create});
}

const checksum = async bytes => Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)),
  byte => byte.toString(16).padStart(2,'0')).join('');

async function verifiedFile(handle, image) {
  const file = await handle.getFile();
  if (file.size !== image.bytes) throw new InvalidCheckpoint('Incorrect saved HDD length');
  return file;
}

export async function openCheckpoint(image) {
  if (slotOf(image) === undefined || typeof image.name !== 'string' || !image.name ||
      !Number.isFinite(image.savedAt) || !Number.isInteger(image.bytes) ||
      image.bytes < 512 || image.bytes > 0xffffffff ||
      !Array.isArray(image.digests) || image.digests.length !== Math.ceil(image.bytes / chunkBytes)) {
    throw new InvalidCheckpoint('Invalid HDD checkpoint metadata');
  }
  const handle = await (await directory()).getFileHandle(fileName(image.slot));
  const file = await verifiedFile(handle, image);
  return {
    size:file.size,
    header:() => file.slice(0,512).arrayBuffer(),
    async read(at) {
      const bytes = await file.slice(at, at + chunkBytes).arrayBuffer();
      if (bytes.byteLength !== Math.min(chunkBytes, image.bytes - at) ||
          await checksum(bytes) !== image.digests[at / chunkBytes]) {
        throw new InvalidCheckpoint('Saved HDD checksum mismatch');
      }
      return bytes;
    },
  };
}

function checkpointCandidates({active, previous}) {
  // A startup fallback can be mounted while the active checkpoint is temporarily unreadable.
  return [active, previous].filter(checkpoint => slotOf(checkpoint) !== undefined);
}

async function firstValidCheckpoint(candidates, verifying = () => {}) {
  for (const checkpoint of candidates) {
    try {
      const image = await openCheckpoint(checkpoint);
      for (let at = 0; at < image.size; at += chunkBytes) {
        const bytes = await image.read(at);
        verifying(at, bytes.byteLength, image.size);
      }
      return checkpoint;
    } catch (error) {
      // Inconclusive I/O failures must abort before changing the catalog or files.
      if (error instanceof InvalidCheckpoint ||
          (error instanceof DOMException && error.name === 'NotFoundError')) continue;
      throw error;
    }
  }
  return null;
}

async function stageCheckpoint(handle, name, bytes, read, verifying) {
  const worker = new Worker(new URL('hdd-storage-worker.js',import.meta.url), {type:'module'});
  let pending, fault;
  function fail(error) {
    fault = error;
    pending?.reject(error);
  }
  worker.onerror = event => { event.preventDefault(); fail(Error(event.message || 'HDD storage Worker stopped')); };
  worker.onmessageerror = () => fail(Error('Invalid HDD storage Worker reply'));
  worker.onmessage = ({data}) => {
    if (data.error) fail(Error(data.error));
    else pending?.resolve();
  };
  const send = async (data, transfer = []) => {
    if (fault) throw fault;
    let timeout;
    try {
      await new Promise((resolve,reject) => {
        pending = {resolve,reject};
        // A terminated Worker can leave a request unanswered without an error event.
        timeout = setTimeout(() => fail(Error('HDD storage Worker timed out')), workerReplyTimeoutMs);
        worker.postMessage(data, transfer);
      });
    } finally {
      clearTimeout(timeout);
      pending = undefined;
    }
  };

  const digests = [];
  try {
    await send({type:'open', handle});
    for (let at = 0; at < bytes; at += chunkBytes) {
      const length = Math.min(chunkBytes, bytes - at);
      const buffer = await read(at, length);
      if (buffer.byteLength !== length) throw Error('Incomplete HDD snapshot');
      digests.push(await checksum(buffer));
      await send({type:'write', at, buffer}, [buffer]);
    }
    await send({type:'finish', bytes});
  } finally { worker.terminate(); }

  const file = await verifiedFile(handle, {bytes});
  for (let at = 0; at < bytes; at += chunkBytes) {
    if (await checksum(await file.slice(at, at + chunkBytes).arrayBuffer()) !== digests[at / chunkBytes]) {
      throw Error('Staged HDD checksum mismatch');
    }
    verifying(at, Math.min(chunkBytes, bytes - at), bytes);
  }
  return {name, bytes, digests, savedAt:Date.now()};
}

export class HddStorage {
  // The loaded revision is a compare-and-swap token across tabs. An older live
  // disk cannot publish over a newer selection even after the write lock frees.
  current;

  async startup(restore) {
    if (!navigator.locks) {
      const current = await readManifest();
      // These choices need no checkpoint file access or writable catalog.
      if (current.active === 'default' || current.active === 'empty') return current;
      throw Error('Restoring a saved HDD requires Web Locks');
    }
    return locked('shared', async () => {
      this.current = await readManifest();
      await restore(this.current);
      return this.current;
    });
  }

  async update(change, afterCommit) {
    return locked('exclusive', async () => {
      const current = await readManifest();
      if (!this.current || current.revision !== this.current.revision) {
        throw Error('HDD startup changed in another tab. Export this disk if needed, then reload');
      }
      const next = await change(current);
      if (next) {
        this.current = await manifest({...next, revision:crypto.randomUUID()});
        await afterCommit?.(this.current);
      }
    });
  }

  select(active) {
    return this.update(async current => {
      if (current.active === active) return;
      const candidates = checkpointCandidates(current);
      // Keep a sole candidate as-is. Verify before dropping a competing generation.
      const previous = candidates.length === 2 ? await firstValidCheckpoint(candidates) : candidates[0] ?? null;
      return {active, previous};
    });
  }

  recover(prepare) {
    return this.update(async current => {
      if (!current.previous) throw Error('No previous HDD checkpoint');
      await prepare(current.previous);
      return {active:current.previous, previous:typeof current.active === 'object' ? current.active : null};
    });
  }

  save(name, bytes, read, verifying, {mountedCheckpoint, unchanged} = {}) {
    let dir;
    const discardUnreferenced = async ({active,previous}) => {
      for (const slot of slots) {
        if (slot !== slotOf(active) && slot !== slotOf(previous)) {
          // Missing files or failed cleanup leave the committed catalog valid.
          await dir.removeEntry(fileName(slot)).catch(() => {});
        }
      }
    };
    return this.update(async current => {
      const previous = await firstValidCheckpoint(checkpointCandidates(current), verifying);
      if (previous && previous === current.active && mountedCheckpoint === this.current.active && unchanged) return;

      dir = await directory(true);
      await discardUnreferenced(current);
      // Keep both catalog references intact throughout staging. A crash can
      // leave at most one unreferenced file, which the next save can recycle.
      const occupied = [slotOf(current.active), slotOf(current.previous)];
      const slot = slots.find(candidate => !occupied.includes(candidate));
      const handle = await dir.getFileHandle(fileName(slot), {create:true});
      const active = {...await stageCheckpoint(handle, name, bytes, read, verifying), slot};
      return {active, previous};
    }, discardUnreferenced).then(() => this.current.active);
  }
}
