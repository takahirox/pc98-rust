const $ = (id) => document.getElementById(id);
const screen = $('screen'), context = screen.getContext('2d', { alpha: false });
const decoder = new TextDecoder();
let core, running = false, audio, outputGain, nextAudio = 0, muted = true, loading = false;
let audioEnergy = 0, scheduledSamples = 0;
const mounted = [null, null], held = new Set();
const controls = ['game', 'hello', 'pause', 'reset', 'sound', 'eject', 'export', 'profile'];
function message(kind = 0) {
  const ptr = core.message(kind), len = core.message_len();
  return decoder.decode(new Uint8Array(core.memory.buffer, ptr, len));
}
function fail(error) {
  $('error').textContent = String(error.message ?? error); $('error').hidden = false;
}
function status(text) { $('status').textContent = text; $('lamp').classList.toggle('active', running); }
function clearError() { $('error').hidden = true; $('error').textContent = ''; }
function render() {
  const ptr = core.frame();
  // frame() can grow Wasm memory: obtain views only after each exported call.
  const pixels = new Uint8ClampedArray(core.memory.buffer, ptr, 640 * 400 * 4);
  context.putImageData(new ImageData(pixels, 640, 400), 0, 0);
}
async function enableAudio() {
  if (!audio) { audio = new AudioContext({ sampleRate: 48000 }); outputGain = audio.createGain(); outputGain.connect(audio.destination); }
  await audio.resume(); muted = false; outputGain.gain.value = running ? 1 : 0;
  $('sound').textContent = 'Mute'; $('sound').setAttribute('aria-pressed', 'true');
}
function playAudio() {
  const count = core.audio_len();
  if (count && audio?.state === 'running' && !muted) {
    const ptr = core.audio_ptr();
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
}
function setRunning(value) {
  running = value; if (outputGain) outputGain.gain.value = running && !muted ? 1 : 0; $('pause').textContent = value ? 'Pause' : 'Resume';
  if (!value) { releaseKeys(); core.audio_clear(); }
  status(value ? 'Running · click screen to type' : 'Paused');
}
function showMedia() {
  const drive = Number($('drive').value);
  $('media-name').textContent = mounted[drive]?.name ?? 'No disk';
  $('eject').disabled = !mounted[drive]; $('export').disabled = !mounted[drive];
}
function insert(bytes, name, drive) {
  if (bytes.length > 4 * 1024 * 1024) throw Error('Floppy image exceeds 4 MiB.');
  if (!/\.(tfd|hdm|xdf|2hd|d88|d98|img)$/i.test(name)) throw Error('Unsupported floppy extension.');
  const ptr = core.input_buffer(bytes.length);
  if (!ptr) throw Error('Unable to allocate floppy buffer.');
  new Uint8Array(core.memory.buffer, ptr, bytes.length).set(bytes);
  if (!core.insert(drive, /\.(d88|d98)$/i.test(name) ? 1 : 0)) throw Error(message(2));
  mounted[drive] = { name, format: /\.(d88|d98)$/i.test(name) ? 'd88' : 'hdm' };
  showMedia();
}
async function loadBytes(bytes, name, drive = Number($('drive').value)) {
  clearError(); insert(bytes, name, drive);
  if (drive === 0) { releaseKeys(); core.reset(); core.audio_clear(); render(); setRunning(true); screen.focus(); }
}
async function bundled(name) {
  if (!core || loading) return;
  loading = true;
  try {
    // Audio is initialized inside the user gesture, before waiting for assets.
    const enable = enableAudio().catch((error) => fail(error));
    const response = await fetch(`media/${name}`);
    if (!response.ok) throw Error(`Cannot load demo: HTTP ${response.status}`);
    $('drive').value = '0';
    await loadBytes(new Uint8Array(await response.arrayBuffer()), name, 0);
    await enable;
  } catch (error) { fail(error); } finally { loading = false; }
}
function exportBytes(drive) {
  const ptr = core.export_disk(drive), len = core.export_len();
  return new Uint8Array(core.memory.buffer, ptr, len).slice();
}
$('game').onclick = () => bundled('startrail.hdm');
$('hello').onclick = () => bundled('hello98.tfd');
$('pause').onclick = () => { if (core) setRunning(!running); };
$('reset').onclick = () => { releaseKeys(); core.reset(); nextAudio = 0; clearError(); setRunning(true); render(); screen.focus(); };
$('sound').onclick = async () => {
  try { if (muted) await enableAudio(); else { muted = true; outputGain.gain.value = 0; $('sound').textContent = 'Enable sound'; $('sound').setAttribute('aria-pressed', 'false'); } }
  catch (error) { fail(error); }
};
$('profile').onchange = () => {
  releaseKeys();
  if (!core.set_profile(Number($('profile').value))) { fail(Error('Unsupported machine profile')); return; }
  $('profile-label').textContent = $('profile').value === '1' ? 'PC-9821Ap DOS · experimental' : 'PC-9801VM · experimental';
  $('display-mode').textContent = $('profile').value === '1'
    ? '640 × 400 · 16 colors · Shinonome Japanese CG · approximate OPN'
    : '640 × 400 · 8 colors · replacement Latin font';
  clearError(); nextAudio = 0; core.audio_clear(); render();
  if (mounted[0]) setRunning(true);
};
$('drive').onchange = showMedia;
$('eject').onclick = () => { const drive = Number($('drive').value); core.eject(drive); mounted[drive] = null; showMedia(); };
$('export').onclick = () => {
  const drive = Number($('drive').value), data = exportBytes(drive);
  const link = document.createElement('a'), url = URL.createObjectURL(new Blob([data]));
  link.href = url; link.download = mounted[drive].name; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};
async function loadFile(file) {
  if (!file || !core) return;
  try { if (file.size > 4 * 1024 * 1024) throw Error('Floppy image exceeds 4 MiB.'); await loadBytes(new Uint8Array(await file.arrayBuffer()), file.name); }
  catch (error) { fail(error); }
}
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
  // Machine-readable inspection of the actual Wasm core for deterministic tests.
  window.pc98 = { core, text: () => message(1), read: (addr) => core.peek(addr),
    stats: () => ({instructions:core.metric(0),reads:core.metric(1),writes:core.metric(2),beepEdges:core.metric(3),fmWrites:core.metric(6),fmKeyEdges:core.metric(7),error:message(),beep:core.beep(),running,audioState:audio?.state,audioGain:outputGain?.gain.value,audioEnergy,scheduledSamples}),
    exportDisk: (drive=0) => Array.from(exportBytes(drive)),
  };
  controls.forEach((id)=>$(id).disabled=false); showMedia(); status('Ready · choose a demo or insert a floppy'); render(); requestAnimationFrame(animate);
} catch (error) { status('Unable to start'); fail(error); }
