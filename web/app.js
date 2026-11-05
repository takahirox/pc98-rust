const $ = (id) => document.getElementById(id);
const screen = $('screen'), context = screen.getContext('2d', { alpha: false });
const decoder = new TextDecoder();
let core, running = false, audio, outputGain, nextAudio = 0, muted = true, loading = false;
let audioEnergy = 0, scheduledSamples = 0;
const mounted = [null, null, null], held = new Set();
let pointerPosition;
let lastDrive = null;
const driveNames = ['Floppy 1', 'Floppy 2', 'IDE HDD'];
const controls = ['game', 'hello', 'pause', 'reset', 'sound', 'eject', 'export', 'profile', 'boot', 'default-hdd'];
function message(kind = 0) {
  const ptr = (core.message(kind) >>> 0), len = core.message_len();
  return decoder.decode(new Uint8Array(core.memory.buffer, ptr, len));
}
function fail(error) {
  $('error').textContent = String(error.message ?? error); $('error').hidden = false;
}
function status(text) { $('status').textContent = text; $('lamp').classList.toggle('active', running); }
function clearError() { $('error').hidden = true; $('error').textContent = ''; }
function render() {
  const ptr = (core.frame() >>> 0);
  // frame() can grow Wasm memory: obtain views only after each exported call.
  const pixels = new Uint8ClampedArray(core.memory.buffer, ptr, 640 * 400 * 4);
  context.putImageData(new ImageData(pixels, 640, 400), 0, 0);
  if ($('profile').value === '1') $('display-mode').textContent = core.metric(13)
    ? '640 × 400 · 256 colors · PEGC'
    : '640 × 400 · 16 colors · Shinonome Japanese CG · approximate OPN';
}
async function enableAudio() {
  if (!audio) { audio = new AudioContext({ sampleRate: 48000 }); outputGain = audio.createGain(); outputGain.connect(audio.destination); }
  await audio.resume(); muted = false; outputGain.gain.value = running ? 1 : 0;
  $('sound').textContent = 'Mute'; $('sound').setAttribute('aria-pressed', 'true');
}
function playAudio() {
  const count = core.audio_len();
  if (count && audio?.state === 'running' && !muted) {
    const ptr = (core.audio_ptr() >>> 0);
    const samples = new Float32Array(core.memory.buffer, ptr, count).slice();
    const buffer = audio.createBuffer(1, count, 48000);
    buffer.copyToChannel(samples, 0);
    audioEnergy += samples.reduce((sum, n) => sum + n * n, 0);
    scheduledSamples += count;
    const source = audio.createBufferSource(); source.buffer = buffer; source.connect(outputGain);
    nextAudio = Math.max(audio.currentTime + 0.025, Math.min(nextAudio, audio.currentTime + 0.12));
    source.start(nextAudio); nextAudio += count / 48000;
  }
  core.audio_clear();
}
function releaseKeys() {
  for (const scan of held) core?.key(scan, 0);
  held.clear(); document.querySelectorAll('.held').forEach((el) => el.classList.remove('held'));
  core?.mouse(0, 0, 0); pointerPosition = undefined;
}
function setRunning(value) {
  running = value; if (outputGain) outputGain.gain.value = running && !muted ? 1 : 0; $('pause').textContent = value ? 'Pause' : 'Resume';
  if (!value) { releaseKeys(); core.audio_clear(); }
  status(value ? 'Running · click screen to type' : 'Paused');
}
function showMedia() {
  const drive = mediaDrive();
  $('media-drive').textContent = drive === null ? 'Auto' : driveNames[drive]
    + (drive === 2 && core.metric(21) === 0 ? ' (requires Ap)' : '');
  $('media-name').textContent = mounted[drive]?.name ?? 'No disk';
  $('media-origin').textContent = mounted[drive] ? (mounted[drive].source === 'bundled' ? 'Bundled' : 'Local file') : '';
  $('eject').disabled = !mounted[drive]; $('export').disabled = !mounted[drive];
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
function insert(bytes, name, drive) {
  checkSize(bytes.length, drive);
  const ptr = (core.input_buffer(bytes.length) >>> 0);
  if (!ptr) throw Error('Unable to allocate media buffer.');
  new Uint8Array(core.memory.buffer, ptr, bytes.length).set(bytes);
  if (!core.insert(drive, /\.(d88|d98)$/i.test(name) ? 1 : 0)) throw Error(message(2));
  mounted[drive] = { name, format: /\.(d88|d98)$/i.test(name) ? 'd88' : 'hdm' };
  lastDrive = drive;
  showMedia();
}
async function loadBytes(bytes, name, drive) {
  clearError(); insert(bytes, name, drive);
  // Floppy insertion is live media access, independent of the next boot.
}
async function bundled(name) {
  if (!core || loading) return;
  loading = true; mediaBusy(true);
  try {
    // Audio is initialized inside the user gesture, before waiting for assets.
    const enable = enableAudio().catch((error) => fail(error));
    const response = await fetch(`media/${name}`);
    if (!response.ok) throw Error(`Cannot load demo: HTTP ${response.status}`);
    $('drive').value = '0';
    await loadBytes(new Uint8Array(await response.arrayBuffer()), name, 0);
    mounted[0].source = 'bundled';
    core.set_boot_source(0); resetMachine();
    await enable;
  } catch (error) { fail(error); } finally { loading = false; mediaBusy(false); }
}
function exportBytes(drive) {
  if (drive === 2) {
    const len = (core.hdi_len() >>> 0);
    if (len > 16 * 1024 * 1024) throw Error('Use exportBlob for large HDI inspection.');
    const bytes = new Uint8Array(len);
    for (let at = 0; at < len; at += chunkBytes) {
      const n = Math.min(chunkBytes, len - at), ptr = (core.hdi_read_chunk(at, n) >>> 0);
      if (!ptr) throw Error(message(2) || 'Unable to read disk export.');
      bytes.set(new Uint8Array(core.memory.buffer, ptr, n), at);
    }
    return bytes;
  }
  const ptr = (core.export_disk(drive) >>> 0), len = core.export_len();
  return new Uint8Array(core.memory.buffer, ptr, len).slice();
}
$('game').onclick = () => bundled('startrail.hdm');
$('hello').onclick = () => bundled('hello98.tfd');
$('pause').onclick = () => { if (core) setRunning(!running); };
function showActiveBoot() {
  const drive = core.peek(0x584) === 0x80 ? 2 : 0;
  $('active-boot').textContent = `Booted: ${driveNames[drive]} · ${mounted[drive]?.name ?? 'No disk'}`;
}
function resetMachine() { releaseKeys(); core.reset(); showActiveBoot(); nextAudio = 0; clearError(); showMedia(); setRunning(true); render(); screen.focus(); }
$('reset').onclick = resetMachine;
$('boot').onchange = () => { core.set_boot_source(Number($('boot').value)); showMedia(); };
$('sound').onclick = async () => {
  try { if (muted) await enableAudio(); else { muted = true; outputGain.gain.value = 0; $('sound').textContent = 'Enable sound'; $('sound').setAttribute('aria-pressed', 'false'); } }
  catch (error) { fail(error); }
};
function selectProfile() {
  releaseKeys();
  if (!core.set_profile(Number($('profile').value))) throw Error('Unsupported machine profile');
  document.title = `pc98-rust · ${$('profile').value === '1' ? 'PC-9821Ap DOS' : 'PC-9801VM'}`;
  $('profile-label').textContent = $('profile').value === '1' ? 'PC-9821Ap DOS · experimental' : 'PC-9801VM · experimental';
  $('display-mode').textContent = $('profile').value === '1'
    ? '640 × 400 · 16 colors · Shinonome Japanese CG · approximate OPN'
    : '640 × 400 · 8 colors · replacement Latin font';
  $('mouse').disabled = $('profile').value !== '1';
  if ($('mouse').disabled) $('mouse').checked = false;
  clearError(); nextAudio = 0; core.audio_clear(); render(); showMedia(); showActiveBoot();
  setRunning(Boolean(mounted[0] || (mounted[2] && $('profile').value === '1')));
}
$('profile').onchange = () => { try { selectProfile(); } catch (error) { fail(error); } };
$('drive').onchange = showMedia;
function mayReplace(drive) {
  return drive !== 2 || !mounted[2] || (core.hdi_dirty_page(0) >>> 0) === 0xffffffff ||
    confirm(`${mounted[2].name} has guest writes. Discard them? Cancel to keep the disk and use Export disk first.`);
}
$('eject').onclick = () => { const drive = mediaDrive(); if (!mounted[drive] || !mayReplace(drive)) return; core.eject(drive); mounted[drive] = null; showMedia(); };
const chunkBytes = 1024 * 1024;
const yieldUI = () => globalThis.scheduler?.yield() ?? new Promise(resolve => setTimeout(resolve, 0));
function mediaBusy(value) {
  for (const id of [...controls, 'file', 'drive']) $(id).disabled = value;
  if (!value) showMedia();
}
async function exportBlob(drive) {
  if (drive !== 2) return new Blob([exportBytes(drive)]);
  // Snapshot the mounted bytes while paused. Local File slices can become
  // unreadable after saving over the input file, even after Blob construction
  // succeeds. Blob snapshots own each bounded chunk independently of that file
  // and of the reusable Wasm transfer buffer.
  const resume = running;
  setRunning(false); mediaBusy(true);
  try {
    const len = (core.hdi_len() >>> 0), parts = [];
    if (!len) throw Error('No HDI mounted.');
    for (let at = 0; at < len; at += chunkBytes) {
      const n = Math.min(chunkBytes, len - at), ptr = (core.hdi_read_chunk(at, n) >>> 0);
      if (!ptr) throw Error(message(2) || 'Unable to read disk export chunk.');
      parts.push(new Blob([new Uint8Array(core.memory.buffer, ptr, n)]));
      status(`Exporting HDI · ${Math.min(100, Math.floor((at + n) * 100 / len))}%`);
      await yieldUI();
    }
    const blob = new Blob(parts, { type: 'application/octet-stream' });
    if (blob.size !== len) throw Error('Incomplete HDI export; please retry.');
    return blob;
  } catch (error) {
    throw Error(`Cannot export HDI: ${error.message}. Disk remains mounted; free browser resources and retry.`);
  } finally { mediaBusy(false); setRunning(resume); }
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
    if (!mayReplace(drive)) return;
    mediaBusy(true);
    if (drive === 2) {
      setRunning(false); suspended = true;
      const transfer = bytes => {
        const ptr = (core.input_buffer(bytes.length) >>> 0);
        if (!ptr) throw Error('Browser memory cannot allocate the HDI transfer chunk; mounted disk retained.');
        new Uint8Array(core.memory.buffer, ptr, bytes.length).set(bytes);
      };
      transfer(new Uint8Array(await file.slice(0, 32).arrayBuffer()));
      if (!core.hdi_begin(file.size)) throw Error(message(2));
      for (let at = 0; at < file.size; at += chunkBytes) {
        transfer(new Uint8Array(await file.slice(at, at + chunkBytes).arrayBuffer()));
        if (!core.hdi_append()) throw Error(message(2));
        status(`Importing HDI · ${Math.min(100, Math.floor((at + chunkBytes) * 100 / file.size))}%`);
      }
      if (!core.hdi_commit()) throw Error(message(2));
      mounted[2] = { name: file.name, format: 'hdi' };
      lastDrive = 2;
      resetMachine();
    } else {
      await loadBytes(new Uint8Array(await file.arrayBuffer()), file.name, drive);
    }
  } catch (error) { core.hdi_cancel(); fail(error); if (suspended) setRunning(resume); }
  finally { loading = false; mediaBusy(false); }
}
async function defaultHdd() {
  if (!core || loading || core.metric(21) !== 1 || !mayReplace(2)) return;
  loading = true; mediaBusy(true);
  const resume = running;
  setRunning(false); clearError(); status('Loading FreeDOS HDD…');
  try {
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
    const transfer = bytes => {
      const ptr = core.input_buffer(bytes.length) >>> 0;
      if (!ptr) throw Error('Browser memory cannot allocate FreeDOS transfer buffer');
      new Uint8Array(core.memory.buffer, ptr, bytes.length).set(bytes);
    };
    transfer(prefix.subarray(0, 32));
    if (!core.hdi_begin(manifest.bytes)) throw Error(message(2));
    transfer(prefix);
    if (!core.hdi_append()) throw Error(message(2));
    const zero = new Uint8Array(chunkBytes);
    for (let at = received; at < manifest.bytes; at += chunkBytes) {
      transfer(zero.subarray(0, Math.min(chunkBytes, manifest.bytes-at)));
      if (!core.hdi_append()) throw Error(message(2));
      if (at % (8*chunkBytes) === received % (8*chunkBytes)) await yieldUI();
    }
    if (!core.hdi_commit()) throw Error(message(2));
    mounted[2] = {name: 'FreeDOS-128MiB.hdi', format: 'hdi', source: 'bundled'};
    lastDrive = 2;
    resetMachine();
  } catch (error) {
    core.hdi_cancel(); setRunning(resume);
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
  core.key(scan, down ? 1 : 0);
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
  core.mouse(delta[0], delta[1], name === 'pointercancel' ? 0 : event.buttons);
  event.preventDefault();
});
screen.addEventListener('pointerleave', () => { pointerPosition = undefined; });
screen.addEventListener('contextmenu', event => { if ($('mouse').checked) event.preventDefault(); });
$('mouse').onchange = () => { core?.mouse(0, 0, 0); pointerPosition = undefined; };
screen.addEventListener('blur',releaseKeys); window.addEventListener('blur',releaseKeys);
document.addEventListener('visibilitychange',()=>{if(document.hidden && core && running)setRunning(false);});
let previous = 0;
function animate(now) {
  if (core && running) {
    // Fixed bounded slices; never catch up an unbounded backlog after tab sleep.
    const slices = Math.max(1, Math.min(2, Math.round((now-previous)/16.667)));
    for(let i=0;i<slices;i++) { core.step(16667); playAudio(); }
    if (core.metric(4)) { setRunning(false); fail(message()); status('Guest stopped · unsupported instruction or BIOS request'); }
    render();
  }
  previous = now; requestAnimationFrame(animate);
}
try {
  const response = await fetch('core.wasm');
  if (!response.ok) throw Error(`Missing Wasm bundle (HTTP ${response.status}). Run npm run build.`);
  const result = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  core = result.instance.exports;
  core.set_boot_source(1);
  selectProfile();
  // Machine-readable inspection of the actual Wasm core for deterministic tests.
  window.pc98 = { core, text: () => message(1), read: (addr) => core.peek(addr),
    stats: () => ({instructions:core.metric(0),reads:core.metric(1),writes:core.metric(2),hddReads:core.metric(9),hddWrites:core.metric(10),mouseEvents:core.metric(11),mouseLatches:core.metric(12),pegc:core.metric(13),pcmWrites:core.metric(14),pcmSamples:core.metric(15),beepEdges:core.metric(3),fmWrites:core.metric(6),fmKeyEdges:core.metric(7),error:message(),beep:core.beep(),running,audioState:audio?.state,audioGain:outputGain?.gain.value,audioEnergy,scheduledSamples}),
    exportDisk: (drive=0) => Array.from(exportBytes(drive)),
    exportBlob: (drive=2) => exportBlob(drive),
  };
  mediaBusy(true); render(); requestAnimationFrame(animate);
  await defaultHdd();
} catch (error) { status('Unable to start'); fail(error); }
