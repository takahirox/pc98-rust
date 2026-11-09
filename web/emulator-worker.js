// The sole Wasm/Machine owner. Every command runs between finite guest slices.
// Requests: {id,type,data}; replies: {id,value|error,state}. Unsolicited frame,
// stopped and fault events never resolve requests. frameAck returns ownership
// of the sole pixel buffer; audio uses its own credit-based MessagePort.
let core, running = false, manual = false, busy = false, timer;
let generation = 0, frameBuffer = new ArrayBuffer(640 * 400 * 4), frameDue = 0, frameRequested = false;
let videoPort, snapshotBuffer = null, videoEpoch = 0, frameSequence = 0;
let inFlightSequence = 0;
const videoStats = {snapshots:0, coalesced:0, snapshotMs:0, copyMs:0, inlineRasterMs:0, guestSliceMs:0, guestSlices:0, maxPreparationMs:0};
function invalidateVideo() {
  ++videoEpoch;
  postMessage({event:'videoEpoch',videoEpoch});
  frameDue = 0;
}
let port, audioEnabled = false, audioState, audioEpoch = 0, flushWaiter;
const audioBuffers = Array.from({length: 6}, () => new ArrayBuffer(1024 * 4));
let sinkStats = {}, sinkStatsAt = 0, transaction = null;
let producedSamples = 0, sentSamples = 0, droppedSamples = 0;
let deadline = 0, diagnostics = false;
// Opt-in bounded host-cost histogram; no guest data or per-instruction logging.
const sliceHistogram = new Uint32Array(501); // 0.1 ms buckets; last bucket >=50 ms.
let tickToken = 0;
let guestDeadline = 0, throughputSample = null, instructionThroughput = 0;
// A task targets 10 ms of device time, independently capped at 100,000 units.
// Retain the deadline if the work cap is reached; no recursive catch-up loop.
function guestSlice() {
  const now = core.metric(5);
  if (guestDeadline <= now) guestDeadline = now + 80000;
  const started = performance.now();
  core.step_scheduler(Math.ceil(guestDeadline-now), 100000);
  const elapsed = performance.now()-started;
  videoStats.guestSliceMs += elapsed; videoStats.guestSlices++;
  if (diagnostics) sliceHistogram[Math.min(500,Math.floor(elapsed*10))]++;
}
const tickTasks = new MessageChannel();
tickTasks.port1.onmessage = ({data:token}) => {
  if (timer === 'task' && token === tickToken) tick();
};
function playingAudio() { return port && audioEnabled && audioState === 'running'; }
function outstandingAudio() { return sentSamples - (sinkStats.audioEpoch === audioEpoch ? sinkStats.consumedSamples : 0); }
const decoder = new TextDecoder();
function message(kind = 0) {
  const ptr = core.message(kind) >>> 0, len = core.message_len();
  return decoder.decode(new Uint8Array(core.memory.buffer, ptr, len));
}
function state() {
  return { generation, metrics: [13, 21, 22].map(id => [id, core.metric(id)]), boot: core.peek(0x584) };
}
function stats() {
  const fields = {instructions:0, reads:1, writes:2, hddReads:9, hddWrites:10,
    mouseEvents:11, mouseLatches:12, pegc:13, pcmWrites:14, pcmSamples:15,
    beepEdges:3, fmWrites:6, fmKeyEdges:7, cycles:5, irqAcks:20,
    soundIrqEdges:23, fmPitch:24, ssgPeriod:25, cpuCycles:26,
    completedInstructions:27, repElements:28, guestSeconds:29, approximatedUnits:30, cpuHz:31};
  const now = performance.now(), instructions = core.metric(27);
  if (throughputSample && instructions < throughputSample.instructions) instructionThroughput = 0;
  else if (throughputSample && now > throughputSample.time) {
    instructionThroughput = (instructions-throughputSample.instructions)*1000/(now-throughputSample.time);
  }
  throughputSample = {time:now, instructions};
  return {...Object.fromEntries(Object.entries(fields).map(([key,id]) => [key, core.metric(id)])),
    sliceHistogram:diagnostics ? Array.from(sliceHistogram) : undefined,
    instructionThroughput, error:message(), beep:core.beep(), running, audioState, audioGain:running && audioEnabled && audioState === 'running' ? 1 : 0,
    audioEnergy:0, scheduledSamples:0, ...sinkStats, generation,
    frameInFlight:videoPort ? !snapshotBuffer : !frameBuffer, videoEpoch, videoMode:videoPort ? 'worker' : 'inline', videoStats:{...videoStats}, audioBuffers:audioBuffers.length, transaction,
    producedSamples, sentSamples, droppedSamples, outstandingSamples:outstandingAudio()};
}
function input(bytes) {
  if (bytes.byteLength > 4 * 1024 * 1024) throw Error('Transfer exceeds 4 MiB');
  const ptr = core.input_buffer(bytes.byteLength) >>> 0;
  if (!ptr) throw Error('Unable to allocate media buffer.');
  new Uint8Array(core.memory.buffer, ptr, bytes.byteLength).set(new Uint8Array(bytes));
}
function exportDisk(drive = 0) {
  if (drive === 2) {
    const len = core.hdi_len() >>> 0;
    if (len > 16 * 1024 * 1024) throw Error('Use exportBlob for large HDI inspection.');
    const bytes = new Uint8Array(len);
    for (let at = 0; at < len; at += 1024 * 1024) {
      const n = Math.min(1024 * 1024, len-at), ptr = core.hdi_read_chunk(at,n) >>> 0;
      if (!ptr) throw Error(message(2));
      bytes.set(new Uint8Array(core.memory.buffer,ptr,n),at);
    }
    return Array.from(bytes);
  }
  const ptr = core.export_disk(drive) >>> 0, len = core.export_len();
  return Array.from(new Uint8Array(core.memory.buffer,ptr,len));
}
function present() {
  if (videoPort ? !snapshotBuffer : !frameBuffer) {
    videoStats.coalesced++;
    frameRequested = true; return;
  }
  frameRequested = false;
  const started = performance.now(), captured = performance.timeOrigin+started, sequence = ++frameSequence;
  if (videoPort) {
    const ptr = core.video_snapshot() >>> 0, length = core.video_snapshot_len();
    const copied = performance.now();
    if (length > snapshotBuffer.byteLength) throw Error('Video snapshot exceeds transfer bound');
    new Uint8Array(snapshotBuffer,0,length).set(new Uint8Array(core.memory.buffer,ptr,length));
    videoStats.snapshotMs += copied-started;
    videoStats.copyMs += performance.now()-copied;
    videoStats.maxPreparationMs = Math.max(videoStats.maxPreparationMs,performance.now()-started);
    videoStats.snapshots++;
    inFlightSequence = sequence;
    videoPort.postMessage({buffer:snapshotBuffer,length,sequence,videoEpoch,captured,
      guestCycles:core.metric(5),...state()},[snapshotBuffer]);
    snapshotBuffer = null;
  } else {
    const ptr = core.frame() >>> 0; // May grow memory: obtain the view afterwards.
    const copied = performance.now();
    new Uint8Array(frameBuffer).set(new Uint8Array(core.memory.buffer,ptr,640*400*4));
    videoStats.inlineRasterMs += copied-started;
    videoStats.copyMs += performance.now()-copied;
    videoStats.maxPreparationMs = Math.max(videoStats.maxPreparationMs,performance.now()-started);
    videoStats.snapshots++;
    postMessage({event:'frame', buffer:frameBuffer,sequence,videoEpoch,captured,
      guestCycles:core.metric(5),...state()}, [frameBuffer]);
    frameBuffer = null;
  }
}
function presentDue() {
  const now = performance.now();
  if (now < frameDue) return;
  // Snapshot capture is cheap, but launching independent rasterization still
  // competes for the host's CPU budget. Refill PCM before either rendering
  // path, especially during guest scene changes. A wall-clock preview deadline
  // must not spend the remaining PCM reserve while guest slices are expensive.
  // Resume with a fresh full snapshot as soon as the reserve recovers.
  if (playingAudio()) {
    // The last credit's cursor predates this guest slice. Account for PCM the
    // sink could consume while Wasm ran, rather than treating that stale cursor
    // as a full reserve. This estimate only gates video, never guest clocks.
    const reserve = Math.max(0, outstandingAudio() - (now-sinkStatsAt)*48);
    // Six filled credits can leave a partially consumed block: 50 ms then
    // suffices because refill is blocked. Otherwise refill toward 60 ms first.
    // The sink reports whether its current output run is missing PCM. Delayed
    // credits can leave a positive reserve estimate even while it emits silence.
    // Bound preview refresh during that condition; recovered streams still wait
    // for their normal reserve, regardless of historical underrun totals.
    const starvedPreview = sinkStats.audioEpoch === audioEpoch && sinkStats.underflowRun > 0 && now >= frameDue+100;
    if (reserve < (audioBuffers.length ? 2880 : 2400) && !starvedPreview) return;
  }
  present(); frameDue = performance.now()+33;
}
function deliverAudio() {
  const count = core.audio_len();
  if (playingAudio()) {
    producedSamples += count;
    let sent = 0;
    const ptr = core.audio_ptr() >>> 0, samples = new Float32Array(core.memory.buffer,ptr,count);
    // Six credits bound both MessagePort traffic and the sink queue. Normal
    // execution stops producing at the watermark, before exhausting credits.
    for (let at = 0; at < count && audioBuffers.length; at += 1024) {
      const buffer = audioBuffers.pop(), n = Math.min(1024,count-at);
      new Float32Array(buffer).set(samples.subarray(at,at+n));
      let checksum;
      if (diagnostics) {
        checksum = 2166136261;
        for (const bits of new Uint32Array(buffer,0,n)) checksum = Math.imul(checksum ^ bits,16777619) >>> 0;
      }
      port.postMessage({type:'pcm', epoch:audioEpoch, buffer, count:n, start:producedSamples-count+at, checksum}, [buffer]);
      sent += n;
    }
    sentSamples += sent; droppedSamples += count-sent;
  }
  core.audio_clear();
}
async function flushAudio() {
  core.audio_clear(); ++audioEpoch;
  producedSamples = sentSamples = droppedSamples = 0; deadline = 0; guestDeadline = 0;
  throughputSample = null; instructionThroughput = 0;
  if (!port) return;
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { flushWaiter = null; reject(Error('AudioWorklet did not acknowledge flush')); }, 2000);
    flushWaiter = () => { clearTimeout(timeout); resolve(); };
    port.postMessage({type:'flush', epoch:audioEpoch});
  });
}
function schedule(delay = 10) {
  if (!running || manual || transaction) {
    if (timer !== 'task') clearTimeout(timer);
    timer = null; ++tickToken; return;
  }
  // A returned credit is a fresh sink cursor. Do not leave its refill behind
  // the polling timer: that spends reserve precisely when the sink needs PCM.
  if (delay === 0 && timer && timer !== 'task') { clearTimeout(timer); timer = null; }
  if (timer) return;
  // Nested timers clamp to >=4 ms in browsers. Paying that after every
  // 10 ms guest slice starves audio when the slice itself needs >6 ms.
  // MessageChannel yields between finite slices without imposing that floor.
  // Keep an actual timer while ahead, so a full sink never spins tasks.
  if (delay === 0) { timer = 'task'; tickTasks.port2.postMessage(++tickToken); }
  else timer = setTimeout(tick,delay);
}
function tick() {
  timer = null;
  const started = performance.now();
  if (!running || busy || transaction) { schedule(); return; }
  try {
    // The device consumes 48 kHz guest PCM. Target 60 ms ahead of its
    // source cursor, including in-flight messages. Timer jitter cannot become
    // a permanent producer deficit, and a slow/suspended sink cannot lose PCM.
    // Each task still executes only one finite 10 ms slice before yielding.
    if (playingAudio()) {
      if (outstandingAudio() < 2880 && audioBuffers.length) guestSlice();
    } else {
      if (!deadline) deadline = started;
      if (started >= deadline) {
        guestSlice();
        // Bounded wall-clock catch-up when muted; no debt after suspension.
        if (core.metric(5) >= guestDeadline) deadline = Math.max(deadline+10, performance.now()-30);
      }
    }
    deliverAudio();
    presentDue();
    if (core.metric(4)) {
      running = false;
      postMessage({event:'stopped', error:message()});
    }
  } catch (error) { running = false; postMessage({event:'fault', error:error.message}); }
  schedule(playingAudio() ? (outstandingAudio() < 2880 && audioBuffers.length ? 0 : 5)
    : (core.metric(5) < guestDeadline ? 0 : Math.max(1,deadline-performance.now())));
}
const calls = new Set(['key','mouse','set_boot_source','set_profile','reset','eject',
  'hdi_begin','hdi_append','hdi_commit','hdi_cancel','hdi_len','hdi_dirty_page']);
async function command(type, data) {
  if (type === 'init') {
    const response = await fetch(new URL('core.wasm', import.meta.url));
    if (!response.ok) throw Error(`Missing Wasm bundle (HTTP ${response.status}). Run npm run build.`);
    core = (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance.exports;
    diagnostics = !!data.diagnostics;
    manual = diagnostics && data.manual; core.set_boot_source(1);
    // Playwright evaluates bounded synchronous probes inside this same Worker.
    // No eval/RPC source execution endpoint is installed in the production UI.
    if (data.diagnostics) {
      let allowance = 0;
      function timedInspection(clocks, cap) {
        if (transaction || busy) throw Error('Inspection overlaps a media/control operation');
        if (!Number.isInteger(clocks) || clocks < 0 || !Number.isInteger(cap) || cap < 0) throw Error('Invalid inspection budget');
        if (cap && allowance <= 0) throw Error('Inspection exceeded profile work budget');
        const done = core.step_scheduler(clocks, Math.min(cap,allowance));
        allowance -= done;
        return done;
      }
      const diagnosticCore = new Proxy({...core},{get:(target,name) => {
        if (name === 'step_scheduler') return timedInspection;
        if (name === 'step') return budget => {
          if (transaction || busy) throw Error('Inspection overlaps a media/control operation');
          if (!Number.isInteger(budget) || budget < 0) throw Error('Invalid inspection budget');
          allowance -= Math.min(budget,100000);
          if (allowance < 0) throw Error('Inspection exceeded profile work budget');
          return core.step(budget);
        };
        return target[name];
      }});
      globalThis.pc98 = {core:diagnosticCore, text:() => message(1), read:addr => core.peek(addr), stats, exportDisk,
        advance:microseconds => {
          if (!Number.isInteger(microseconds) || microseconds < 0 || microseconds > 100000) throw Error('Invalid guest-time budget');
          const end = core.metric(5) + microseconds*8;
          while (core.metric(5) < end && !core.metric(4)) {
            timedInspection(Math.min(80000,end-core.metric(5)),100000);
          }
          return microseconds;
        },
        transitionStart:() => ({guest:core.metric(29), wall:performance.now()}),
        transitionDuration:start => ({guestSeconds:core.metric(29)-start.guest, wallMs:performance.now()-start.wall}),
        beginInspection:() => { allowance = Math.ceil(16000000 * core.metric(31) / 8000000); },
        endInspection:() => { if (running && !manual) deliverAudio(); present(); },
      };
    }
    return;
  }
  if (!core) throw Error('Emulator is not initialized');
  if (type === 'running') {
    invalidateVideo(); running = data; deadline = 0;
    if (!data) {
      await flushAudio();
      // Paused workers stop ticking. Request this epoch's full frame now, or
      // coalesce it until the invalidated in-flight snapshot returns its credit.
      present();
    }
    schedule(); return;
  }
  if (type === 'sound') { audioEnabled = data.enabled; audioState = data.state; await flushAudio(); return; }
  if (type === 'audioPort') {
    port = data;
    port.onmessage = ({data:m}) => {
      if (m.buffer) audioBuffers.push(m.buffer);
      if (m.stats && m.epoch === audioEpoch) { sinkStats = m.stats; sinkStatsAt = performance.now(); }
      if (m.type === 'flushed' && m.epoch === audioEpoch) {
        flushWaiter?.(); flushWaiter = null;
      }
      // A short missing-PCM run can begin and recover between guest slices.
      // Service its bounded preview deadline on the fresh sink notification,
      // while no Wasm slice is executing, instead of losing that edge at tick().
      const activeStarvation = m.type === 'stats' && m.epoch === audioEpoch && m.stats?.underflowRun > 0
        && playingAudio() && running && !manual && !busy && !transaction && performance.now() >= frameDue+100;
      if (activeStarvation) presentDue();
      if (m.epoch === audioEpoch && playingAudio() && outstandingAudio() < 2880 && audioBuffers.length) schedule(0);
    };
    port.start(); await flushAudio(); return;
  }
  if (type === 'videoPort') {
    videoPort = data;
    frameBuffer = null; snapshotBuffer = new ArrayBuffer(547632);
    const connected = videoPort;
    videoPort.onmessage = ({data:m}) => {
      if (videoPort !== connected) return;
      if (!snapshotBuffer && m.sequence === inFlightSequence && m.buffer?.byteLength === 547632) {
        snapshotBuffer = m.buffer;
        // A credit never runs rasterization or a guest slice synchronously.
        if (frameRequested) {
          if (running) schedule(0);
          else present();
        }
      }
    };
    videoPort.onmessageerror = () => {
      if (videoPort === connected) postMessage({event:'renderFault',error:'Invalid render credit message'});
    };
    videoPort.start(); invalidateVideo(); present(); return;
  }
  if (type === 'videoRefresh') { invalidateVideo(); present(); return; }
  if (type === 'videoFallback') {
    if (!videoPort) return; // Duplicate fault notifications cannot mint pixel credits.
    videoPort.close(); videoPort = null;
    // A failed renderer may own the old buffer. Only this fault path replaces it.
    snapshotBuffer = null;
    if (!frameBuffer) frameBuffer = new ArrayBuffer(640*400*4);
    invalidateVideo(); present(); return;
  }
  if (type === 'stats') return stats();
  if (type === 'text') return message(1);
  if (type === 'read') return core.peek(data);
  if (type === 'snapshot') {
    if (data && transaction) throw Error('Media operation already in progress');
    invalidateVideo();
    transaction = data ? 'export' : null; schedule(); return core.hdi_len() >>> 0;
  }
  if (type === 'input') { if (transaction === 'export') throw Error('Disk snapshot in progress'); input(data); return; }
  if (type === 'insert') {
    if (transaction) throw Error('Media operation in progress');
    invalidateVideo();
    if (!core.insert(data.drive,data.format)) throw Error(message(2)); return;
  }
  if (type === 'export') {
    if (data === 2) throw Error('Use chunked HDI export');
    const ptr = core.export_disk(data) >>> 0, len = core.export_len();
    return new Uint8Array(core.memory.buffer,ptr,len).slice().buffer;
  }
  if (type === 'readChunk') {
    if (transaction !== 'export') throw Error('No disk snapshot in progress');
    const [at,n] = data;
    if (n > 1024*1024 || n < 1) throw Error('Invalid export chunk');
    const ptr = core.hdi_read_chunk(at,n) >>> 0;
    if (!ptr) throw Error(message(2) || 'Unable to read disk export chunk.');
    return new Uint8Array(core.memory.buffer,ptr,n).slice().buffer;
  }
  if (type === 'call') {
    const {name,args} = data;
    if (!calls.has(name)) throw Error('Unsupported core command');
    if (transaction && !['hdi_append','hdi_commit','hdi_cancel','hdi_len','hdi_dirty_page','key','mouse'].includes(name)) throw Error('Media operation in progress');
    if (['reset','set_profile','set_boot_source','eject','hdi_begin','hdi_commit','hdi_cancel'].includes(name)) invalidateVideo();
    if (['reset','set_profile'].includes(name)) { await flushAudio(); generation++; }
    if (transaction && (name === 'key' && args[1] || name === 'mouse' && args.some(Boolean))) throw Error('Input is paused for media transfer');
    const value = core[name](...args);
    if (name === 'hdi_begin' && value) transaction = 'import';
    if (name === 'hdi_commit' && value || name === 'hdi_cancel') transaction = null;
    if (['hdi_begin','hdi_append','hdi_commit'].includes(name) && !value) throw Error(message(2));
    if (['reset','set_profile','eject'].includes(name)) { frameDue = 0; present(); }
    schedule(); return value;
  }
  throw Error(`Unknown Worker command: ${type}`);
}
let commands = Promise.resolve();
onmessage = ({data:m}) => {
  if (m.type === 'frameAck') {
    if (m.data.byteLength === 640*400*4 && !frameBuffer) {
      frameBuffer = m.data;
      if (frameRequested) {
        if (running && playingAudio()) schedule(0);
        else present();
      }
    }
    return;
  }
  commands = commands.then(async () => {
    busy = true;
    try {
      const value = await command(m.type,m.data);
      postMessage({id:m.id, value, state:core && state()}, value instanceof ArrayBuffer ? [value] : []);
    } catch (error) { postMessage({id:m.id, error:error.message}); }
    finally { busy = false; schedule(); }
  });
};
