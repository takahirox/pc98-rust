const $ = (id) => document.getElementById(id);
const screen = $('screen'), context = screen.getContext('2d', { alpha: false });
let core, worker, running = false, audio, outputGain, audioNode, audioSetup, muted = true, loading = false;
let contextLost = false;
let renderWorker, videoEpoch = 0, presentedSequence = 0;
let rendererStartupMs = 0, rendererFailure = null;
let renderStats = {}, presentationStats = {frames:0, stale:0, putMs:0, latencyMs:0, maxLatencyMs:0};
let generation = 0, pendingFrame, presenting = false, workerFailed = false;
const pending = new Map(); let requestId = 0;
const metrics = new Map(); let bootByte = 0;
const mounted = [null, null, null], held = new Set();
let pointerPosition;
let lastDrive = null;
const driveNames = ['Floppy 1', 'Floppy 2', 'IDE HDD'];
const controls = ['game', 'hello', 'pause', 'reset', 'sound', 'eject', 'export', 'profile', 'boot', 'default-hdd'];
function fail(error) {
  $('error').textContent = String(error.message ?? error); $('error').hidden = false;
}
function status(text) { $('status').textContent = text; $('lamp').classList.toggle('active', running); }
function clearError() { $('error').hidden = true; $('error').textContent = ''; }
function acceptState(state) {
  if (!state || state.generation < generation) return;
  generation = state.generation; bootByte = state.boot;
  for (const [id,value] of state.metrics) metrics.set(id,value);
}
function rpc(type, data, transfer = []) {
  if (workerFailed) return Promise.reject(Error('Emulation Worker stopped. Reload to restart.'));
  return new Promise((resolve,reject) => {
    const id = ++requestId;
    pending.set(id,{resolve,reject});
    try { worker.postMessage({id,type,data},transfer); }
    catch (error) { pending.delete(id); reject(error); }
  });
}
function workerFault(error) {
  workerFailed = true; running = false; ++videoEpoch; pendingFrame = null;
  if (outputGain) outputGain.gain.value = 0;
  worker?.terminate(); renderWorker?.terminate(); renderWorker = null;
  for (const waiter of pending.values()) waiter.reject(error);
  pending.clear(); mediaBusy(true); status('Worker stopped · reload to restart'); fail(error);
}
function recycleFrame(frame) {
  if (frame.renderer) {
    if (frame.renderer === renderWorker) renderWorker.postMessage({type:'frameAck',buffer:frame.buffer,sequence:frame.sequence},[frame.buffer]);
  } else if (!workerFailed) worker.postMessage({type:'frameAck',data:frame.buffer},[frame.buffer]);
}
function render() {
  presenting = false;
  if (!pendingFrame) return;
  const frame = pendingFrame; pendingFrame = null;
  try {
    if (frame.generation === generation && frame.videoEpoch === videoEpoch && frame.sequence > presentedSequence && !contextLost) {
      presentedSequence = frame.sequence;
      const began = performance.now();
      context.putImageData(new ImageData(new Uint8ClampedArray(frame.buffer),640,400),0,0);
      presentationStats.putMs += performance.now()-began;
      const latency = performance.timeOrigin+performance.now()-frame.captured;
      presentationStats.latencyMs += latency;
      presentationStats.maxLatencyMs = Math.max(presentationStats.maxLatencyMs,latency);
      presentationStats.frames++;
      if ($('profile').value === '1') $('display-mode').textContent = core?.metric(13)
        ? '640 × 400 · 256 colors · PEGC'
        : '640 × 400 · 16 colors · Shinonome Japanese CG · approximate OPN';
    }
    else presentationStats.stale++;
  } finally { recycleFrame(frame); }
}
function queueFrame(frame) {
  if (frame.videoEpoch < videoEpoch || frame.generation < generation || frame.sequence <= presentedSequence
      || pendingFrame && frame.sequence <= pendingFrame.sequence) {
    presentationStats.stale++; recycleFrame(frame); return;
  }
  if (frame.videoEpoch >= videoEpoch) {
    videoEpoch = frame.videoEpoch;
    acceptState(frame);
  }
  if (pendingFrame) recycleFrame(pendingFrame);
  pendingFrame = frame;
  if (!presenting) { presenting = true; requestAnimationFrame(render); }
}
function rendererFault(candidate, reason = 'Render Worker stopped') {
  if (!candidate || renderWorker !== candidate) return;
  rendererFailure = String(reason);
  renderWorker = null; candidate.terminate();
  if (pendingFrame?.renderer === candidate) pendingFrame = null;
  rpc('videoFallback').catch(fail);
}
async function startRenderer() {
  const started = performance.now();
  let candidate;
  try {
    candidate = new Worker(new URL('render-worker.js',import.meta.url),{type:'module'});
    await new Promise((resolve,reject) => {
      const timeout = setTimeout(()=>reject(Error('Render Worker startup timeout')),5000);
      candidate.onerror = event => { event.preventDefault(); clearTimeout(timeout); reject(Error(event.message || 'Render Worker failed')); };
      candidate.onmessageerror = () => { clearTimeout(timeout); reject(Error('Invalid Render Worker message')); };
      candidate.onmessage = ({data:m}) => {
        clearTimeout(timeout);
        if (m.event === 'ready') resolve(); else reject(Error(m.error || 'Render Worker startup failed'));
      };
      candidate.postMessage({type:'init',diagnostics:new URLSearchParams(location.search).has('diagnostics')});
    });
    rendererStartupMs = performance.now()-started;
    renderWorker = candidate;
    candidate.onerror = event => { event.preventDefault(); rendererFault(candidate,event.message); };
    candidate.onmessageerror = () => rendererFault(candidate,'Invalid Render Worker message');
    candidate.onmessage = ({data:m}) => {
      if (renderWorker !== candidate) return;
      if (m.event === 'fault') { rendererFault(candidate,m.error); return; }
      if (m.event === 'frame') { renderStats = m.renderStats; queueFrame({...m,renderer:candidate}); }
    };
    const channel = new MessageChannel();
    candidate.postMessage({type:'connect',port:channel.port1},[channel.port1]);
    await rpc('videoPort',channel.port2,[channel.port2]);
  } catch (error) {
    rendererFailure = error.message; rendererStartupMs = performance.now()-started;
    candidate?.terminate(); renderWorker = null;
    await rpc('videoFallback');
  }
}
screen.addEventListener('contextlost',event => { event.preventDefault(); contextLost = true; });
screen.addEventListener('contextrestored',() => { contextLost = false; rpc('videoRefresh').catch(fail); });
async function soundState() {
  if (outputGain) outputGain.gain.value = 0;
  await rpc('sound',{enabled:!muted, state:audio?.state});
  if (outputGain) outputGain.gain.value = running && !muted && audio?.state === 'running' ? 1 : 0;
}
async function enableAudio() {
  if (!audio) {
    audio = new AudioContext({sampleRate:48000});
    outputGain = audio.createGain(); outputGain.gain.value = 0; outputGain.connect(audio.destination);
    audioSetup = (async () => {
      await audio.audioWorklet.addModule(new URL('audio-worklet.js',import.meta.url));
      audioNode = new AudioWorkletNode(audio,'pc98-audio',{outputChannelCount:[1]});
      audioNode.onprocessorerror = () => { muted = true; soundState().catch(fail); fail('AudioWorklet stopped. Reload to enable sound again.'); };
      const channel = new MessageChannel();
      audioNode.port.postMessage({type:'connect',port:channel.port1},[channel.port1]);
      audioNode.connect(outputGain);
      await rpc('audioPort',channel.port2,[channel.port2]);
      audio.onstatechange = () => soundState().catch(fail);
    })();
  }
  // resume is invoked before awaiting module loading to retain gesture permission.
  const resumed = audio.resume();
  try { await Promise.all([audioSetup,resumed]); }
  catch (error) { muted = true; await audio.close(); audio = null; throw error; }
  muted = false; await soundState();
  $('sound').textContent = 'Mute'; $('sound').setAttribute('aria-pressed','true');
}
function releaseKeys() {
  for (const scan of held) core?.key(scan, 0).catch(fail);
  held.clear(); document.querySelectorAll('.held').forEach((el) => el.classList.remove('held'));
  core?.mouse(0, 0, 0).catch(fail); pointerPosition = undefined;
}
async function setRunning(value) {
  running = value; $('pause').textContent = value ? 'Pause' : 'Resume';
  if (outputGain) outputGain.gain.value = running && !muted ? 1 : 0;
  if (!value) releaseKeys();
  await rpc('running',value);
  status(value ? 'Running · click screen to type' : 'Paused');
}
function showMedia() {
  const drive = mediaDrive();
  $('media-drive').textContent = drive === null ? 'Auto' : driveNames[drive]
    + (drive === 2 && core.metric(21) === 0 ? ' (requires Ap)' : '');
  $('media-name').textContent = mounted[drive]?.name ?? 'No disk';
  $('media-origin').textContent = mounted[drive] ? (mounted[drive].source === 'bundled' ? 'Bundled' : 'Local file') : '';
  $('eject').disabled = loading || !mounted[drive]; $('export').disabled = loading || !mounted[drive];
  // Rust owns the preference. VM always boots its floppy, retaining Ap's choice.
  const boot = core.metric(21) === 1 && core.metric(22) ? 2 : 0;
  $('boot').value = String(core.metric(22));
  $('boot').disabled = loading || core.metric(21) !== 1;
  $('default-hdd').disabled = loading || core.metric(21) !== 1;
  $('boot-source').textContent = `Reset boots: ${driveNames[boot]} · ${mounted[boot]?.name ?? 'No disk'}`;
}
function mediaDrive() { return $('drive').value === 'auto' ? lastDrive : Number($('drive').value); }
function resolveDrive(name) {
  const hdi = /\.hdi$/i.test(name), floppy = /\.(tfd|hdm|xdf|2hd|d88|d98|img)$/i.test(name);
  if (!hdi && !floppy) throw Error('Unsupported media extension. Use HDI or a supported floppy image.');
  if (hdi && core.metric(21) !== 1) throw Error('IDE requires PC-9821Ap DOS. Select that machine to insert an HDI.');
  const drive = $('drive').value === 'auto' ? (hdi ? 2 : 0) : Number($('drive').value);
  if (drive === 2 && !hdi) throw Error('IDE HDD requires an HDI image. Choose Auto or a floppy drive for this image.');
  if (drive !== 2 && hdi) throw Error('HDI requires IDE HDD. Choose Auto or IDE HDD for this image.');
  return drive;
}
function checkSize(size, drive) {
  if (size > (drive === 2 ? 0xffffffff : 4 * 1024 * 1024)) throw Error(drive === 2
    ? 'HDI file extent must be below 4 GiB (32-bit container/browser addressing).'
    : 'Floppy image exceeds 4 MiB.');
}
async function insert(bytes, name, drive) {
  checkSize(bytes.length,drive);
  await rpc('input',bytes.buffer,[bytes.buffer]);
  await rpc('insert',{drive,format:/\.(d88|d98)$/i.test(name) ? 1 : 0});
  mounted[drive] = {name,format:/\.(d88|d98)$/i.test(name) ? 'd88' : 'hdm'};
  lastDrive = drive; showMedia();
}
async function loadBytes(bytes, name, drive) {
  clearError(); await insert(bytes, name, drive);
  // Floppy insertion is live media access, independent of the next boot.
}
async function bundled(name) {
  if (!core || loading) return;
  loading = true; mediaBusy(true);
  try {
    // Audio is initialized inside the user gesture, before waiting for assets.
    const enable = enableAudio().then(() => null,error => error);
    const response = await fetch(`media/${name}`);
    if (!response.ok) throw Error(`Cannot load demo: HTTP ${response.status}`);
    $('drive').value = '0';
    await loadBytes(new Uint8Array(await response.arrayBuffer()), name, 0);
    mounted[0].source = 'bundled';
    await core.set_boot_source(0); await resetMachine();
    const audioError = await enable;
    if (audioError) fail(audioError);
  } catch (error) { fail(error); } finally { loading = false; mediaBusy(false); }
}
$('game').onclick = () => bundled('startrail.hdm');
$('hello').onclick = () => bundled('hello98.tfd');
$('pause').onclick = () => exclusive(() => setRunning(!running));
function showActiveBoot() {
  const drive = bootByte === 0x80 ? 2 : 0;
  $('active-boot').textContent = `Booted: ${driveNames[drive]} · ${mounted[drive]?.name ?? 'No disk'}`;
}
async function resetMachine() {
  releaseKeys();
  if (outputGain) outputGain.gain.value = 0;
  if (pendingFrame) { recycleFrame(pendingFrame); pendingFrame = null; }
  await core.reset(); showActiveBoot(); clearError(); showMedia();
  await setRunning(!document.hidden); screen.focus();
}
async function exclusive(operation) {
  if (!core || loading || workerFailed) return;
  loading = true; mediaBusy(true);
  try { await operation(); } catch (error) { fail(error); }
  finally { loading = false; mediaBusy(false); }
}
$('reset').onclick = () => exclusive(resetMachine);
$('boot').onchange = () => {
  const source = Number($('boot').value);
  exclusive(async () => { await core.set_boot_source(source); showMedia(); });
};
$('sound').onclick = () => exclusive(async () => {
  clearError();
  if (muted) await enableAudio();
  else { muted = true; await soundState(); $('sound').textContent = 'Enable sound'; $('sound').setAttribute('aria-pressed','false'); }
});
async function selectProfile() {
  releaseKeys();
  const profile = $('profile').value;
  if (!await core.set_profile(Number(profile))) throw Error('Unsupported machine profile');
  $('profile').value = profile;
  document.title = `pc98-rust · ${$('profile').value === '1' ? 'PC-9821Ap DOS' : 'PC-9801VM'}`;
  $('profile-label').textContent = $('profile').value === '1' ? 'PC-9821Ap DOS · experimental' : 'PC-9801VM · experimental';
  $('display-mode').textContent = $('profile').value === '1'
    ? '640 × 400 · 16 colors · Shinonome Japanese CG · approximate OPN'
    : '640 × 400 · 8 colors · replacement Latin font';
  $('mouse').disabled = $('profile').value !== '1';
  if ($('mouse').disabled) $('mouse').checked = false;
  clearError(); showMedia(); showActiveBoot();
  await setRunning(!document.hidden && Boolean(mounted[0] || (mounted[2] && $('profile').value === '1')));
}
$('profile').onchange = () => exclusive(selectProfile);
$('drive').onchange = showMedia;
async function mayReplace(drive) {
  return drive !== 2 || !mounted[2] || ((await core.hdi_dirty_page(0)) >>> 0) === 0xffffffff ||
    confirm(`${mounted[2].name} has guest writes. Discard them? Cancel to keep the disk and use Export disk first.`);
}
$('eject').onclick = () => exclusive(async () => {
  const drive = mediaDrive(), resume = running;
  if (!mounted[drive]) return;
  if (drive === 2) await setRunning(false);
  try {
    if (!await mayReplace(drive)) return;
    await core.eject(drive); mounted[drive] = null; showMedia();
  } finally { if (drive === 2 && !workerFailed) await setRunning(resume && !document.hidden); }
});
const chunkBytes = 1024 * 1024;
function mediaBusy(value) {
  value ||= loading || workerFailed;
  for (const id of [...controls, 'file', 'drive']) $(id).disabled = value;
  if (!value && !workerFailed) showMedia();
}
async function exportBlob(drive) {
  if (drive !== 2) return new Blob([await rpc('export',drive)]);
  // Snapshot the mounted bytes while paused. Local File slices can become
  // unreadable after saving over the input file, even after Blob construction
  // succeeds. Blob snapshots own each bounded chunk independently of that file
  // and of the reusable Wasm transfer buffer.
  const resume = running;
  await setRunning(false); mediaBusy(true);
  try {
    const len = await rpc('snapshot',true), parts = [];
    if (!len) throw Error('No HDI mounted.');
    for (let at = 0; at < len; at += chunkBytes) {
      const n = Math.min(chunkBytes,len-at);
      parts.push(new Blob([await rpc('readChunk',[at,n])]));
      status(`Exporting HDI · ${Math.min(100, Math.floor((at + n) * 100 / len))}%`);
    }
    const blob = new Blob(parts, { type: 'application/octet-stream' });
    if (blob.size !== len) throw Error('Incomplete HDI export; please retry.');
    return blob;
  } catch (error) {
    throw Error(`Cannot export HDI: ${error.message}. Disk remains mounted; free browser resources and retry.`);
  } finally { await rpc('snapshot',false); mediaBusy(false); await setRunning(resume && !document.hidden); }
}
$('export').onclick = async () => {
  const drive = mediaDrive();
  if (loading || !mounted[drive]) return;
  loading = true;
  try {
    clearError();
    const data = await exportBlob(drive);
    const link = document.createElement('a'), url = URL.createObjectURL(data);
    link.href = url; link.download = mounted[drive].name; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (error) { fail(error); } finally { loading = false; mediaBusy(false); }
};
async function loadFile(file) {
  if (!file || !core || loading) return;
  loading = true;
  const resume = running;
  let suspended = false;
  try {
    clearError();
    const drive = resolveDrive(file.name);
    checkSize(file.size, drive);
    mediaBusy(true);
    if (drive === 2) {
      await setRunning(false); suspended = true;
      if (!await mayReplace(drive)) { await setRunning(resume && !document.hidden); return; }
      const transfer = bytes => rpc('input',bytes.buffer,[bytes.buffer]);
      await transfer(new Uint8Array(await file.slice(0,32).arrayBuffer()));
      await core.hdi_begin(file.size);
      for (let at = 0; at < file.size; at += chunkBytes) {
        await transfer(new Uint8Array(await file.slice(at, at + chunkBytes).arrayBuffer()));
        await core.hdi_append();
        status(`Importing HDI · ${Math.min(100, Math.floor((at + chunkBytes) * 100 / file.size))}%`);
      }
      await core.hdi_commit();
      mounted[2] = { name: file.name, format: 'hdi' };
      lastDrive = 2;
      await resetMachine();
    } else {
      await loadBytes(new Uint8Array(await file.arrayBuffer()), file.name, drive);
    }
  } catch (error) { await core.hdi_cancel().catch(() => {}); fail(error); if (suspended && !workerFailed) await setRunning(resume && !document.hidden); }
  finally { loading = false; mediaBusy(false); }
}
async function defaultHdd() {
  if (!core || loading || core.metric(21) !== 1) return;
  loading = true; mediaBusy(true);
  const resume = running;
  try {
    await setRunning(false);
    if (!await mayReplace(2)) { await setRunning(resume && !document.hidden); return; }
    clearError(); status('Loading FreeDOS HDD…');
    const response = await fetch('freedos/manifest.json');
    if (!response.ok) throw Error(`Cannot load FreeDOS manifest: HTTP ${response.status}`);
    const manifest = await response.json();
    if (manifest.bytes !== 128 * 1024 * 1024 + 4096 ||
        !Number.isInteger(manifest.prefix_bytes) ||
        manifest.prefix_bytes <= 32 || manifest.prefix_bytes > chunkBytes ||
        manifest.prefix_bytes % 65536 ||
        JSON.stringify(manifest.geometry) !== '[512,16,32,512]') throw Error('Invalid FreeDOS template geometry/size');
    const template = await fetch('freedos/desktop.bin');
    if (!template.ok) throw Error(`Cannot load FreeDOS template: HTTP ${template.status}`);
    // Bound even a misconfigured server response before any large allocation.
    const reader = template.body.getReader(), parts = [];
    let received = 0;
    try {
      while (true) {
        const {done, value} = await reader.read();
        if (done) break;
        received += value.length;
        if (received > manifest.prefix_bytes) throw Error('FreeDOS template exceeds expected size');
        parts.push(value);
      }
    } finally { await reader.cancel(); }
    if (received !== manifest.prefix_bytes) throw Error('Truncated FreeDOS template');
    const prefix = new Uint8Array(received);
    let offset = 0;
    for (const part of parts) { prefix.set(part, offset); offset += part.length; }
    const digest = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', prefix)),
      byte => byte.toString(16).padStart(2, '0')).join('');
    if (digest !== manifest.prefix_sha256) throw Error('FreeDOS template checksum mismatch');
    const transfer = bytes => rpc('input',bytes.buffer,[bytes.buffer]);
    await transfer(prefix.slice(0,32));
    await core.hdi_begin(manifest.bytes);
    await transfer(prefix);
    await core.hdi_append();
    const zero = new Uint8Array(chunkBytes);
    for (let at = received; at < manifest.bytes; at += chunkBytes) {
      await transfer(zero.slice(0, Math.min(chunkBytes, manifest.bytes-at)));
      await core.hdi_append();
    }
    await core.hdi_commit();
    mounted[2] = {name: 'FreeDOS-128MiB.hdi', format: 'hdi', source: 'bundled'};
    lastDrive = 2;
    await resetMachine();
  } catch (error) {
    await core.hdi_cancel().catch(() => {}); if (!workerFailed) await setRunning(resume && !document.hidden);
    fail(`${error.message}. Disk retained. Use Restore FreeDOS HDD to retry, or insert local media.`);
  } finally { loading = false; mediaBusy(false); }
}
$('default-hdd').onclick = defaultHdd;
$('file').onchange = (e) => { loadFile(e.target.files[0]); e.target.value = ''; };
$('drop').ondragover = (e) => { e.preventDefault(); $('drop').classList.add('dragging'); };
$('drop').ondragleave = () => $('drop').classList.remove('dragging');
$('drop').ondrop = (e) => { e.preventDefault(); $('drop').classList.remove('dragging'); loadFile(e.dataTransfer.files[0]); };
const rows = [
  [['STOP',0x60],['COPY',0x61],...Array.from({length:10},(_,i)=>[`F${i+1}`,0x62+i])],
  [['Esc',0],...Array.from('1234567890', (ch,i)=>[ch,i+1]),['-',11],['^',12],['¥',13],['BS',14]],
  [['Tab',15],...Array.from('QWERTYUIOP', (ch,i)=>[ch,0x10+i]),['@',0x1a],['[',0x1b],['Return',0x1c]],
  [['Ctrl',0x74],['Caps',0x71],...Array.from('ASDFGHJKL',(ch,i)=>[ch,0x1d+i]),[';',0x26],[':',0x27],[']',0x28]],
  [['Shift',0x70],...Array.from('ZXCVBNM',(ch,i)=>[ch,0x29+i]),[',',0x30],['.',0x31],['/',0x32],['_',0x33]],
  [['Kana',0x72],['GRPH',0x73],['NFER',0x51],['Space',0x34],['XFER',0x35],['←',0x3b],['↑',0x3a],['↓',0x3d],['→',0x3c]],
  [['Home',0x3e],['Help',0x3f],['Roll up',0x36],['Roll down',0x37],['Ins',0x38],['Del',0x39],['7',0x42],['8',0x43],['9',0x44],['4',0x46],['5',0x47],['6',0x48],['1',0x4a],['2',0x4b],['3',0x4c],['0',0x4e]],
];
function key(scan, down) {
  if (!core || !running) return;
  if (down) held.add(scan); else held.delete(scan);
  core.key(scan, down ? 1 : 0).catch(fail);
}
for (const row of rows) {
  const element = document.createElement('div'); element.className = 'keyrow';
  for (const [label, scan] of row) {
    const button = document.createElement('button'); button.textContent = label; button.dataset.scan = scan;
    button.onpointerdown = (e) => { e.preventDefault(); button.setPointerCapture(e.pointerId); button.classList.add('held'); key(scan, true); };
    button.onpointerup = button.onpointercancel = () => { button.classList.remove('held'); key(scan, false); };
    button.onkeydown = (e) => { if (e.code === 'Space' || e.code === 'Enter') { e.preventDefault(); key(scan, true); } };
    button.onkeyup = (e) => { if (e.code === 'Space' || e.code === 'Enter') { e.preventDefault(); key(scan, false); } };
    element.append(button);
  }
  $('keyboard').append(element);
}
const codes = {Escape:0,Backspace:14,Tab:15,Enter:0x1c,Space:0x34,ShiftLeft:0x70,ShiftRight:0x70,ControlLeft:0x74,ControlRight:0x74,AltLeft:0x73,AltRight:0x35,MetaLeft:0x51,CapsLock:0x71,ArrowUp:0x3a,ArrowLeft:0x3b,ArrowRight:0x3c,ArrowDown:0x3d,Insert:0x38,Delete:0x39,Home:0x3e,End:0x3f,PageUp:0x37,PageDown:0x36,Minus:0xb,Equal:0xc,Backslash:0xd,BracketLeft:0x1a,BracketRight:0x1b,Semicolon:0x26,Quote:0x27,Comma:0x30,Period:0x31,Slash:0x32,IntlBackslash:0x33,PrintScreen:0x61,Pause:0x60};
for (const [start,chars] of [[0x10,'QWERTYUIOP'],[0x1d,'ASDFGHJKL'],[0x29,'ZXCVBNM']]) Array.from(chars).forEach((ch,i)=>codes[`Key${ch}`]=start+i);
Array.from('1234567890').forEach((ch,i)=>codes[`Digit${ch}`]=i+1);
for(let i=1;i<=10;i++) codes[`F${i}`]=0x61+i;
for(const [code,scan] of Object.entries({Numpad7:0x42,Numpad8:0x43,Numpad9:0x44,Numpad4:0x46,Numpad5:0x47,Numpad6:0x48,Numpad1:0x4a,Numpad2:0x4b,Numpad3:0x4c,Numpad0:0x4e,NumpadDecimal:0x50,NumpadEnter:0x1c,NumpadAdd:0x49,NumpadSubtract:0x40,NumpadMultiply:0x45,NumpadDivide:0x41})) codes[code]=scan;
for (const [name,down] of [['keydown',true],['keyup',false]]) screen.addEventListener(name,(e)=>{const scan=codes[e.code];if(scan!==undefined){e.preventDefault();key(scan,down);}});
for (const name of ['pointermove', 'pointerdown', 'pointerup', 'pointercancel']) screen.addEventListener(name, (event) => {
  if (!core || !running || !$('mouse').checked || $('profile').value !== '1') return;
  const bounds = screen.getBoundingClientRect();
  const point = [event.clientX * 640 / bounds.width, event.clientY * 400 / bounds.height];
  const delta = pointerPosition ? point.map((n, i) => Math.max(-127, Math.min(127, Math.round(n - pointerPosition[i])))) : [0, 0];
  pointerPosition = point;
  if (name === 'pointerdown') { screen.focus(); screen.setPointerCapture(event.pointerId); }
  core.mouse(delta[0], delta[1], name === 'pointercancel' ? 0 : event.buttons).catch(fail);
  event.preventDefault();
});
screen.addEventListener('pointerleave', () => { pointerPosition = undefined; });
screen.addEventListener('contextmenu', event => { if ($('mouse').checked) event.preventDefault(); });
$('mouse').onchange = () => { core?.mouse(0, 0, 0).catch(fail); pointerPosition = undefined; };
screen.addEventListener('blur',releaseKeys); window.addEventListener('blur',releaseKeys);
document.addEventListener('visibilitychange',()=>{if(document.hidden && core && running)setRunning(false).catch(fail);});
try {
  worker = new Worker(new URL('emulator-worker.js',import.meta.url),{type:'module'});
  worker.onerror = event => { event.preventDefault(); workerFault(Error(event.message || 'Emulation Worker failed')); };
  worker.onmessageerror = () => workerFault(Error('Invalid Emulation Worker message'));
  worker.onmessage = ({data:m}) => {
    if (workerFailed) return;
    if (m.event === 'renderFault') { rendererFault(renderWorker,m.error); return; }
    if (m.event === 'fault') { workerFault(Error(m.error)); return; }
    if (m.event === 'stopped') {
      setRunning(false).catch(fail); fail(m.error); status('Guest stopped · unsupported instruction or BIOS request'); return;
    }
    if (m.event === 'videoEpoch') {
      videoEpoch = Math.max(videoEpoch,m.videoEpoch);
      if (pendingFrame && pendingFrame.videoEpoch < videoEpoch) { recycleFrame(pendingFrame); pendingFrame = null; }
      return;
    }
    acceptState(m.state);
    if (m.event === 'frame') {
      queueFrame(m);
      return;
    }
    const waiter = pending.get(m.id);
    if (!waiter) return;
    pending.delete(m.id);
    if (m.error) waiter.reject(Error(m.error)); else waiter.resolve(m.value);
  };
  const params = new URLSearchParams(location.search);
  await rpc('init',{diagnostics:params.has('diagnostics'),manual:params.has('manual'),jit:params.get('jit')==='1'});
  if (!(params.has('diagnostics') && params.get('renderer') === 'inline')) await startRenderer();
  // This client posts commands and caches metrics; Wasm exports stay in the Worker.
  core = new Proxy({}, {get:(_,name) => name === 'metric' ? id => metrics.get(id) :
    (...args) => rpc('call',{name,args})});
  await selectProfile();
  window.pc98 = {text:() => rpc('text'), read:addr => rpc('read',addr), stats:async () => ({...await rpc('stats'),rendererStartupMs,rendererFailure,renderStats:{...renderStats},presentationStats:{...presentationStats}}),
    exportBlob:async (drive=2) => {
      if (loading) throw Error('Media operation already in progress');
      loading = true; mediaBusy(true);
      try { return await exportBlob(drive); } finally { loading = false; mediaBusy(false); }
    },
  };
  if (params.has('diagnostics')) window.pc98.audio = {
    suspend:async () => { await audio.suspend(); await soundState(); },
    resume:async () => { await audio.resume(); await soundState(); },
  };
  await defaultHdd();
} catch (error) { mediaBusy(true); status('Unable to start'); fail(error); }
