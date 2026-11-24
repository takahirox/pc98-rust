import {Presenter,defaultPreset,presets} from './presenter.js';
const $ = (id) => document.getElementById(id);
const screen = $('screen');
const presenter = new Presenter(screen,$('crt-screen'),$('screen-surface'),
  text => {
    $('presentation-status').textContent = text;
    const warning = $('display-warning');
    warning.hidden = !text.includes('showing original pixels');
    warning.textContent = warning.hidden ? '' : text;
  },new URLSearchParams(location.search).has('diagnostics'));
$('presentation').value = defaultPreset; $('phosphor-mask').checked = false;
try {
  const saved = JSON.parse(localStorage.getItem('pc98-presentation') || 'null');
  if (saved && typeof saved.preset === 'string' && Object.hasOwn(presets,saved.preset) && typeof saved.mask === 'boolean') {
    $('presentation').value = saved.preset; $('phosphor-mask').checked = saved.mask;
  }
} catch { /* Storage is optional in private/embedded browsers. */ }
function presentationSettings() {
  presenter.setPreset($('presentation').value,$('phosphor-mask').checked);
  try { localStorage.setItem('pc98-presentation',JSON.stringify({preset:presenter.preset,mask:presenter.mask})); } catch { /* Optional. */ }
}
$('presentation').onchange = presentationSettings;
$('phosphor-mask').onchange = presentationSettings;
presenter.setPreset($('presentation').value,$('phosphor-mask').checked);
const display = $('drop'), notices = $('notices'), keyboardPanel = $('keyboard-panel');
const drives = document.querySelector('.drives');
const keyboardToggles = [$('toggle-keyboard'), $('fullscreen-keyboard')];
function toggleKeyboard(open) {
  releaseKeys();
  keyboardPanel.hidden = !open;
  for (const toggle of keyboardToggles) toggle.setAttribute('aria-expanded', String(open));
}
for (const toggle of keyboardToggles) toggle.onclick = () => toggleKeyboard(keyboardPanel.hidden);
const keyboardScroll = $('keyboard-scroll');
for (const [id, direction] of [['keys-up', -1], ['keys-down', 1]]) {
  $(id).onclick = () => keyboardScroll.scrollBy(0, direction * Math.max(44, keyboardScroll.clientHeight - 44));
}

let fullscreenFromKeyboard = false;
function fullscreenActive() {
  return document.fullscreenElement === display || display.classList.contains('full-window');
}
function syncFullscreen() {
  toggleKeyboard(false);
  const active = fullscreenActive();
  if (active) {
    notices.tabIndex = 0;
    display.append(notices, keyboardPanel);
  } else {
    const keyboardHome = $('keyboard-home');
    notices.removeAttribute('tabindex');
    drives.before(notices);
    keyboardHome.append(keyboardPanel);
  }
  for (const element of [document.querySelector('.site-header'), ...display.parentElement.children]) {
    if (element !== display) element.inert = active;
  }
  const button = $(active ? 'exit-fullscreen' : 'fullscreen');
  (fullscreenFromKeyboard ? button : screen).focus({preventScroll:true});
}
$('fullscreen').onclick = async event => {
  fullscreenFromKeyboard = event.detail === 0;
  try { await display.requestFullscreen(); }
  catch {
    display.classList.add('full-window');
    syncFullscreen();
  }
};
document.addEventListener('fullscreenchange', syncFullscreen);
async function exitFullscreen() {
  if (document.fullscreenElement === display) await document.exitFullscreen();
  else {
    display.classList.remove('full-window');
    syncFullscreen();
  }
}
$('exit-fullscreen').onclick = () => exitFullscreen().catch(fail);
document.addEventListener('keydown', event => {
  if (event.key === 'Escape' && fullscreenActive()) {
    event.preventDefault(); event.stopPropagation(); exitFullscreen().catch(fail);
  }
}, true);
let core, worker, running = false, audio, outputGain, audioNode, audioSetup, muted = true, loading = false;
let activePlaybackButton;
let contextLost = false;
let renderWorker, videoEpoch = 0, presentedSequence = 0;
let rendererStartupMs = 0, rendererFailure = null;
let renderStats = {}, presentationStats = {frames:0, stale:0, putMs:0, latencyMs:0, maxLatencyMs:0, coalesced:0, missedSlots:0};
let generation = 0, pendingFrame, presenting = false, workerFailed = false;
const framePeriod = 1000/60;
const presentationTolerance = 2;
const captureLead = 10;
let nextPresentation = 0;
const pending = new Map(); let requestId = 0;
const metrics = new Map(); let bootByte = 0;
const mounted = [null, null, null], held = new Set();
const pendingModifiers = new Set();
let pointerPosition, pointerButtons = 0;
new ResizeObserver(() => { pointerPosition = undefined; }).observe(screen);
const driveNames = ['Floppy 1', 'Floppy 2', 'IDE HDD'];
const mediaPicker = $('media-picker'), fileInput = $('file');
const driveButtons = [...document.querySelectorAll('[data-drive]')];
let pickerDrive = 'auto', fileDrive = 'auto';
const controls = ['game', 'hello', 'demos', 'pause', 'reset', 'sound', 'profile', 'file'];
function fail(error) {
  $('error').textContent = String(error.message ?? error); $('error').hidden = false;
}
function status(text, mediaProgress = false) {
  $('status').textContent = text;
  $('lamp').classList.toggle('active', running);
  const progress = $('media-progress');
  progress.hidden = !mediaProgress;
  progress.textContent = mediaProgress ? text : '';
}
function clearError() { $('error').hidden = true; $('error').textContent = ''; }
function acceptState(state) {
  if (!state || state.generation < generation) return;
  generation = state.generation; bootByte = state.boot;
  for (const [id,value] of state.metrics) metrics.set(id,value);
  const mode = metrics.get(21) === 0 ? '640 × 400 · 8 colors · replacement Latin font'
    : metrics.get(13) ? '640 × 400 · 256 colors · PEGC'
      : '640 × 400 · 16 colors · Shinonome Japanese CG';
  const label = $('display-mode');
  if (label.textContent !== mode) label.textContent = mode;
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
  pending.clear(); syncControls(); status('Worker stopped'); fail(`${error.message}. Reload to restart.`);
}
function recycleFrame(frame, captureAt) {
  if (frame.renderer) {
    if (frame.renderer === renderWorker) renderWorker.postMessage({type:'frameAck',buffer:frame.buffer,sequence:frame.sequence,captureAt},[frame.buffer]);
  } else if (!workerFailed) worker.postMessage({type:'frameAck',data:frame.buffer,captureAt},[frame.buffer]);
}
function render(timestamp) {
  if (pendingFrame && running && !contextLost && timestamp+presentationTolerance < nextPresentation) {
    requestAnimationFrame(render);
    return;
  }
  presenting = false;
  if (!pendingFrame) return;
  const frame = pendingFrame; pendingFrame = null;
  let captureAt;
  try {
    if (frame.generation === generation && frame.videoEpoch === videoEpoch && frame.sequence > presentedSequence && !contextLost) {
      presentedSequence = frame.sequence;
      const began = performance.now();
      presenter.present(new ImageData(new Uint8ClampedArray(frame.buffer),640,400),frame);
      presentationStats.putMs += performance.now()-began;
      const latency = performance.timeOrigin+performance.now()-frame.captured;
      presentationStats.latencyMs += latency;
      presentationStats.maxLatencyMs = Math.max(presentationStats.maxLatencyMs,latency);
      presentationStats.frames++;
      if (running && nextPresentation) presentationStats.missedSlots += Math.max(0,Math.floor((timestamp-nextPresentation)/framePeriod));
      nextPresentation = (nextPresentation && timestamp-nextPresentation < framePeriod
        ? nextPresentation : timestamp) + framePeriod;
      // Start capture before the next presentation slot. Use the actual
      // callback time as the origin for the delay calculated from rAF.
      if (running) captureAt = performance.timeOrigin+began
        +Math.max(0,nextPresentation-timestamp-captureLead);
    }
    else presentationStats.stale++;
  } finally { recycleFrame(frame,captureAt); }
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
  if (pendingFrame) { presentationStats.coalesced++; recycleFrame(pendingFrame); }
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
screen.addEventListener('contextlost',event => { event.preventDefault(); contextLost = true; presenter.invalidate(); });
screen.addEventListener('contextrestored',() => { contextLost = false; rpc('videoRefresh').catch(fail); });
function applyVolume() {
  const percent = Number($('volume').value);
  $('volume-level').value = `${percent}%`;
  $('volume').setAttribute('aria-valuetext', `${percent}%`);
  if (outputGain) outputGain.gain.value = running && !muted && audio?.state === 'running' ? percent / 100 : 0;
}
$('volume').oninput = applyVolume;
async function soundState() {
  $('sound').setAttribute('aria-pressed', String(muted));
  $('sound').title = muted ? (audio ? 'Unmute' : 'Enable sound') : 'Mute';
  if (outputGain) outputGain.gain.value = 0;
  await rpc('sound',{enabled:!muted, state:audio?.state});
  applyVolume();
}
async function enableAudio() {
  if (!audio) {
    audio = new AudioContext({sampleRate:48000});
    outputGain = audio.createGain(); outputGain.gain.value = 0; outputGain.connect(audio.destination);
    audioSetup = (async () => {
      await audio.audioWorklet.addModule(new URL('audio-worklet.js',import.meta.url));
      audioNode = new AudioWorkletNode(audio,'pc98-audio',{outputChannelCount:[2]});
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
  catch (error) {
    muted = true; await audio.close(); audio = null;
    applyVolume();
    throw Error(`Cannot enable sound: ${error.message}. Try the sound button again.`);
  }
  muted = false; await soundState();
}
function releaseKeys() {
  pendingModifiers.clear();
  for (const scan of held) core?.key(scan, 0).catch(fail);
  held.clear(); document.querySelectorAll('.held').forEach((el) => el.classList.remove('held'));
  releaseMouse();
}
function releaseMouse() {
  if (pointerButtons) core?.mouse(0, 0, 0).catch(fail);
  pointerButtons = 0;
  pointerPosition = undefined;
}
async function setRunning(value, label = value ? 'Running' : 'Paused', mediaProgress = false) {
  running = value; $('pause').textContent = value ? 'Pause' : 'Resume';
  applyVolume();
  if (!value) releaseKeys();
  await rpc('running',value);
  status(label, mediaProgress);
}
function showPicker() {
  const name = driveNames[pickerDrive], disk = mounted[pickerDrive];
  $('media-title').textContent = name ?? 'Insert media';
  $('picker-disk').hidden = !disk;
  $('picker-disk').textContent = disk?.name ?? '';
  $('media-hint').textContent = pickerDrive === 'auto' ? 'Hard disk images go to IDE HDD. Floppy images go to Floppy 1.'
    : pickerDrive === 2 ? 'Choose an HDI or NHD image. Replacing the disk restarts the machine.'
      : 'Choose a floppy image to insert while the guest is running.';
  $('choose-file').disabled = loading || workerFailed || (pickerDrive === 2 && core?.metric(21) !== 1);
  if (pickerDrive === 2 && core?.metric(21) !== 1) $('media-hint').textContent = 'Switch to PC-9821Ap to use this drive.';
  $('eject').hidden = $('export').hidden = !disk;
  $('eject').disabled = $('export').disabled = loading || workerFailed || !disk;
  if (name) {
    $('choose-file').setAttribute('aria-label', `Choose local file for ${name}`);
    $('eject').setAttribute('aria-label', `Eject ${name}`);
    $('export').setAttribute('aria-label', `Export ${name}`);
  } else $('choose-file').removeAttribute('aria-label');
}
function syncControls() {
  const ap = core?.metric(21) === 1;
  const busy = !core || loading || workerFailed;
  for (const id of controls) {
    const control = $(id);
    control.disabled = busy && (control !== activePlaybackButton || workerFailed);
  }
  for (const button of driveButtons) {
    const drive = Number(button.dataset.drive);
    button.disabled = busy || (drive === 2 && !ap && !mounted[2]);
    if (Number.isNaN(drive)) continue;
    const name = $('media-name-' + drive);
    const disk = mounted[drive];
    name.textContent = disk?.name ?? 'Empty';
    if (drive === 2 && !ap) {
      // Keep the availability message visible even when the filename is long.
      name.textContent = disk ? `Requires Ap · ${disk.name}` : 'Requires Ap';
    }
    name.title = name.textContent;
    button.textContent = disk ? 'Change…' : 'Insert…';
    button.setAttribute('aria-label', `${disk ? 'Change' : 'Insert'} ${driveNames[drive]}`);
  }
  $('mouse-setting').hidden = !ap;
  $('mouse').disabled = !ap || workerFailed;
  $('boot').disabled = $('default-hdd').disabled = busy || !ap;
  const preference = core?.metric(22);
  if (preference !== undefined) {
    const boot = ap && preference ? 2 : 0;
    if (!busy) $('boot').value = boot === 2 ? '1' : '0';
    $('boot-source').textContent = `Reset boots: ${driveNames[boot]} · ${mounted[boot]?.name ?? 'Empty'}`;
  }
  showPicker();
}
for (const button of driveButtons) {
  button.onclick = () => {
    pickerDrive = button.dataset.drive === 'auto' ? 'auto' : Number(button.dataset.drive);
    showPicker();
  };
}
for (const panel of document.querySelectorAll('[popover]')) {
  panel.addEventListener('beforetoggle', event => { if (event.newState === 'open') releaseKeys(); });
}
$('choose-file').onclick = () => {
  fileDrive = pickerDrive;
  fileInput.click();
};
fileInput.onchange = async () => {
  const file = fileInput.files[0];
  fileInput.value = '';
  if (!file) return;
  const trigger = $('drive-' + fileDrive);
  if (mediaPicker.matches(':popover-open')) mediaPicker.hidePopover();
  // The drive button is disabled during import, which can send focus to the body.
  let restoreFocus = [document.body, trigger].includes(document.activeElement);
  const focusMoved = () => { restoreFocus = false; };
  document.addEventListener('focusin', focusMoved, {once:true});
  try {
    await loadFile(file, fileDrive, {focusScreen:false});
  } finally {
    document.removeEventListener('focusin', focusMoved);
    if (restoreFocus) trigger.focus({preventScroll:true});
  }
};
fileInput.oncancel = () => {
  if (mediaPicker.matches(':popover-open')) $('choose-file').focus({preventScroll:true});
};
function resolveDrive(name, target) {
  const hardDisk = /\.(hdi|nhd)$/i.test(name), floppy = /\.(tfd|hdm|xdf|2hd|d88|d98|img)$/i.test(name);
  if (!hardDisk && !floppy) throw Error('Unsupported media extension. Use HDI, NHD or a supported floppy image.');
  const drive = target === 'auto' ? (hardDisk ? 2 : 0) : target;
  if (hardDisk && core.metric(21) !== 1) throw Error('IDE requires PC-9821Ap DOS for hard disk images.');
  if (drive === 2 && !hardDisk) throw Error('IDE HDD requires an HDI or NHD image. Choose Insert media or a floppy drive for this image.');
  if (drive !== 2 && hardDisk) throw Error('Hard disk images require IDE HDD. Choose Insert media or IDE HDD for this image.');
  return drive;
}
function checkSize(size, drive) {
  if (size > (drive === 2 ? 0xffffffff : 4 * 1024 * 1024)) throw Error(drive === 2
    ? 'Hard disk image extent must be below 4 GiB (32-bit container/browser addressing).'
    : 'Floppy image exceeds 4 MiB.');
}
async function insert(bytes, name, drive) {
  checkSize(bytes.length,drive);
  await rpc('input',bytes.buffer,[bytes.buffer]);
  await rpc('insert',{drive,format:/\.(d88|d98)$/i.test(name) ? 1 : 0});
  mounted[drive] = {name};
}
function bundled(name) {
  $('demo-picker').hidePopover();
  return exclusive(async () => {
    clearError();
    // Audio is initialized inside the user gesture, before waiting for assets.
    const enable = enableAudio().then(() => null,error => error);
    const response = await fetch(`media/${name}`);
    if (!response.ok) throw Error(`Cannot load demo: HTTP ${response.status}`);
    await insert(new Uint8Array(await response.arrayBuffer()), name, 0);
    await core.set_boot_source(0); await resetMachine();
    const audioError = await enable;
    if (audioError) fail(audioError);
  });
}
$('game').onclick = () => bundled('startrail.hdm');
$('hello').onclick = () => bundled('hello98.tfd');
$('pause').onclick = event => exclusive(() => setRunning(!running), event.currentTarget);
function showActiveBoot() {
  const drive = bootByte === 0x80 ? 2 : bootByte === 0x90 ? 0 : null;
  const disk = mounted[drive];
  $('active-boot').textContent = disk ? `Booted: ${driveNames[drive]} · ${disk.name}` : 'No active boot disk';
}
async function resetMachine({focusScreen = true} = {}) {
  releaseKeys();
  if (outputGain) outputGain.gain.value = 0;
  if (pendingFrame) { recycleFrame(pendingFrame); pendingFrame = null; }
  await core.reset(); showActiveBoot(); clearError();
  await setRunning(!document.hidden);
  if (focusScreen && !panelOpen()) screen.focus({preventScroll:true});
}
async function exclusive(operation, playbackButton) {
  if (!core || loading || workerFailed) return;
  // Native disabling blurs the button. The transaction guard also blocks
  // activation of the focusable button while its action is pending.
  activePlaybackButton = playbackButton;
  playbackButton?.setAttribute('aria-disabled', 'true');
  loading = true; syncControls();
  try { await operation(); } catch (error) { fail(error); }
  finally {
    playbackButton?.removeAttribute('aria-disabled');
    activePlaybackButton = undefined;
    loading = false; syncControls();
  }
}
$('reset').onclick = () => exclusive(resetMachine);
$('boot').onchange = () => {
  const source = Number($('boot').value);
  return exclusive(() => core.set_boot_source(source));
};
$('sound').onclick = event => exclusive(async () => {
  clearError();
  if (muted) await enableAudio();
  else { muted = true; await soundState(); }
}, event.currentTarget);
async function selectProfile() {
  releaseKeys();
  const profile = $('profile').value;
  try {
    if (!await core.set_profile(Number(profile))) throw Error('Unsupported machine profile');
  } finally { $('profile').value = String(core.metric(21)); }
  const ap = $('profile').value === '1';
  document.title = `pc98-rust · ${ap ? 'PC-9821Ap DOS' : 'PC-9801VM'}`;
  if (!ap) {
    $('mouse').checked = false;
    display.classList.remove('guest-mouse');
  }
  clearError(); showActiveBoot();
  const hasMedia = Boolean(mounted[0] || (mounted[2] && ap));
  await setRunning(!document.hidden && hasMedia, hasMedia ? undefined : 'No boot disk');
}
$('profile').onchange = () => exclusive(selectProfile);
async function mayReplace(drive) {
  return drive !== 2 || !mounted[2] || ((await core.hdi_dirty_page(0)) >>> 0) === 0xffffffff ||
    confirm(`${mounted[2].name} has guest writes. Discard them? Cancel to keep the disk and use Export disk first.`);
}
$('eject').onclick = () => {
  const drive = pickerDrive;
  mediaPicker.hidePopover();
  return exclusive(async () => {
    if (!mounted[drive]) return;
    const resume = running;
    if (drive === 2) await setRunning(false);
    try {
      if (!await mayReplace(drive)) return;
      await core.eject(drive); mounted[drive] = null;
      clearError();
    } finally { if (drive === 2 && !workerFailed) await setRunning(resume && !document.hidden); }
  });
};
const chunkBytes = 1024 * 1024;
async function exportBlob(drive) {
  if (drive !== 2) return new Blob([await rpc('export',drive)]);
  // Snapshot the mounted bytes while paused. Local File slices can become
  // unreadable after saving over the input file, even after Blob construction
  // succeeds. Blob snapshots own each bounded chunk independently of that file
  // and of the reusable Wasm transfer buffer.
  const resume = running;
  await setRunning(false, 'Exporting HDD…', true);
  try {
    const len = await rpc('snapshot',true), parts = [];
    if (!len) throw Error('No hard disk image mounted.');
    for (let at = 0; at < len; at += chunkBytes) {
      const n = Math.min(chunkBytes,len-at);
      parts.push(new Blob([await rpc('readChunk',[at,n])]));
      status(`Exporting HDD · ${Math.min(100, Math.floor((at + n) * 100 / len))}%`, true);
    }
    const blob = new Blob(parts, { type: 'application/octet-stream' });
    if (blob.size !== len) throw Error('Incomplete hard disk export; please retry.');
    return blob;
  } catch (error) {
    throw Error(`Cannot export hard disk: ${error.message}. Disk remains mounted; free browser resources and retry.`);
  } finally { await rpc('snapshot',false); await setRunning(resume && !document.hidden); }
}
$('export').onclick = () => {
  const drive = pickerDrive;
  mediaPicker.hidePopover();
  return exclusive(async () => {
    if (!mounted[drive]) return;
    clearError();
    const data = await exportBlob(drive);
    const link = document.createElement('a'), url = URL.createObjectURL(data);
    link.href = url; link.download = mounted[drive].name; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  });
};
function loadFile(file, target, resetOptions) {
  if (!file) return;
  return exclusive(async () => {
    clearError();
    const drive = resolveDrive(file.name, target);
    checkSize(file.size, drive);
    if (drive !== 2) {
      // Floppy insertion is live access, independent of the next boot.
      await insert(new Uint8Array(await file.arrayBuffer()), file.name, drive);
      return;
    }
    const resume = running;
    await setRunning(false);
    try {
      if (!await mayReplace(drive)) { await setRunning(resume && !document.hidden); return; }
      status('Importing HDD…', true);
      const transfer = bytes => rpc('input',bytes.buffer,[bytes.buffer]);
      await transfer(new Uint8Array(await file.slice(0,512).arrayBuffer()));
      await core.hdi_begin(file.size);
      for (let at = 0; at < file.size; at += chunkBytes) {
        await transfer(new Uint8Array(await file.slice(at, at + chunkBytes).arrayBuffer()));
        await core.hdi_append();
        status(`Importing HDD · ${Math.min(100, Math.floor((at + chunkBytes) * 100 / file.size))}%`, true);
      }
      await core.hdi_commit();
      mounted[2] = {name: file.name};
      await resetMachine(resetOptions);
    } catch (error) {
      await core.hdi_cancel().catch(() => {});
      if (!workerFailed) await setRunning(resume && !document.hidden);
      throw error;
    }
  });
}
function defaultHdd(showProgress = false) {
  if (core?.metric(21) !== 1) return;
  return exclusive(async () => {
    const resume = running;
    try {
      await setRunning(false);
      if (!await mayReplace(2)) { await setRunning(resume && !document.hidden); return; }
      clearError(); status('Loading FreeDOS HDD…', showProgress);
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
      mounted[2] = {name: 'FreeDOS-128MiB.hdi'};
      await resetMachine();
    } catch (error) {
      await core.hdi_cancel().catch(() => {});
      if (!workerFailed) await setRunning(resume && !document.hidden);
      throw Error(`${error.message}. Disk retained. Use Restore FreeDOS HDD to retry, or insert local media.`);
    }
  });
}
$('default-hdd').onclick = () => {
  $('settings-panel').hidePopover();
  return defaultHdd(true);
};
for (const target of [display, ...document.querySelectorAll('[data-drop-drive]')]) {
  target.ondragover = event => {
    if (!event.dataTransfer.types.includes('Files')) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = fileInput.disabled ? 'none' : 'copy';
    target.classList.toggle('dragging', !fileInput.disabled);
  };
  target.ondragleave = event => {
    if (!target.contains(event.relatedTarget)) target.classList.remove('dragging');
  };
  target.ondrop = event => {
    event.preventDefault();
    target.classList.remove('dragging');
    if (!fileInput.disabled) {
      if (mediaPicker.matches(':popover-open')) mediaPicker.hidePopover();
      void loadFile(event.dataTransfer.files[0], target === display ? 'auto' : Number(target.dataset.dropDrive));
    }
  };
}
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
  if (down) held.add(scan); else if (!held.delete(scan)) return;
  core.key(scan, down ? 1 : 0).catch(fail);
}
for (const row of rows) {
  const element = document.createElement('div'); element.className = 'keyrow';
  for (const [label, scan] of row) {
    const button = document.createElement('button'); button.textContent = label; button.dataset.scan = scan;
    button.onpointerdown = (e) => { e.preventDefault(); button.setPointerCapture(e.pointerId); button.classList.add('held'); key(scan, true); };
    button.onpointerup = button.onpointercancel = button.onlostpointercapture = () => { button.classList.remove('held'); key(scan, false); };
    button.onkeydown = (e) => { if (e.code === 'Space' || e.code === 'Enter') { e.preventDefault(); key(scan, true); } };
    button.onkeyup = (e) => { if (e.code === 'Space' || e.code === 'Enter') { e.preventDefault(); key(scan, false); } };
    button.onblur = () => { if (held.has(scan)) key(scan, false); };
    element.append(button);
  }
  $('keyboard').append(element);
}
const codes = {Escape:0,Backspace:14,Tab:15,Enter:0x1c,Space:0x34,ShiftLeft:0x70,ShiftRight:0x70,ControlLeft:0x74,ControlRight:0x74,AltLeft:0x73,AltRight:0x35,MetaLeft:0x51,CapsLock:0x71,ArrowUp:0x3a,ArrowLeft:0x3b,ArrowRight:0x3c,ArrowDown:0x3d,Insert:0x38,Delete:0x39,Home:0x3e,End:0x3f,PageUp:0x37,PageDown:0x36,Minus:0xb,Equal:0xc,Backslash:0xd,BracketLeft:0x1a,BracketRight:0x1b,Semicolon:0x26,Quote:0x27,Comma:0x30,Period:0x31,Slash:0x32,IntlBackslash:0x33,PrintScreen:0x61,Pause:0x60};
for (const [start,chars] of [[0x10,'QWERTYUIOP'],[0x1d,'ASDFGHJKL'],[0x29,'ZXCVBNM']]) Array.from(chars).forEach((ch,i)=>codes[`Key${ch}`]=start+i);
Array.from('1234567890').forEach((ch,i)=>codes[`Digit${ch}`]=i+1);
for(let i=1;i<=10;i++) codes[`F${i}`]=0x61+i;
for(const [code,scan] of Object.entries({Numpad7:0x42,Numpad8:0x43,Numpad9:0x44,Numpad4:0x46,Numpad5:0x47,Numpad6:0x48,Numpad1:0x4a,Numpad2:0x4b,Numpad3:0x4c,Numpad0:0x4e,NumpadDecimal:0x50,NumpadEnter:0x1c,NumpadAdd:0x49,NumpadSubtract:0x40,NumpadMultiply:0x45,NumpadDivide:0x41})) codes[code]=scan;
let captureTab = false;
screen.addEventListener('focus', () => { captureTab = false; });
function panelOpen() { return Boolean(document.querySelector('.panel:popover-open')); }
function captureScreenKeys() {
  captureTab = true;
  for (const code of pendingModifiers) key(codes[code],true);
  pendingModifiers.clear();
}
screen.addEventListener('pointerdown', () => {
  screen.focus({preventScroll:true});
  if (panelOpen()) captureTab = false;
  else captureScreenKeys();
});
screen.addEventListener('keydown', event => {
  if (event.defaultPrevented || panelOpen()) return;
  if (event.code === 'F12') {
    event.preventDefault();
    const playback = $('pause').disabled ? $('presentation') : $('pause');
    (fullscreenActive() ? $('exit-fullscreen') : playback).focus({preventScroll:true});
    return;
  }
  if (!running || (event.code === 'Tab' && !captureTab)) return;
  const scan = codes[event.code];
  if (scan === undefined) return;
  if (!captureTab) {
    // Hold modifiers locally until typing commits to guest input.
    if (['Shift','Control','Alt','AltGraph','Meta'].includes(event.key)) {
      pendingModifiers.add(event.code);
      return;
    }
    captureScreenKeys();
  }
  event.preventDefault();
  key(scan,true);
});
screen.addEventListener('keyup', event => {
  if (pendingModifiers.delete(event.code)) return;
  const scan = codes[event.code];
  if (!held.has(scan)) return;
  event.preventDefault();
  key(scan,false);
});
for (const name of ['pointermove', 'pointerdown', 'pointerup', 'pointercancel']) screen.addEventListener(name, (event) => {
  if (!core || !running || !$('mouse').checked || $('profile').value !== '1' || panelOpen()) return;
  const bounds = screen.getBoundingClientRect();
  const point = [(event.clientX-bounds.left) * screen.width / bounds.width, (event.clientY-bounds.top) * screen.height / bounds.height];
  const delta = pointerPosition ? point.map((n, i) => Math.max(-127, Math.min(127, Math.round(n - pointerPosition[i])))) : [0, 0];
  pointerPosition = point;
  if (name === 'pointerdown') { screen.focus(); screen.setPointerCapture(event.pointerId); }
  pointerButtons = name === 'pointercancel' ? 0 : event.buttons;
  core.mouse(delta[0], delta[1], pointerButtons).catch(fail);
  event.preventDefault();
});
screen.addEventListener('pointerleave', () => { pointerPosition = undefined; });
screen.addEventListener('contextmenu', event => { if ($('mouse').checked) event.preventDefault(); });
$('mouse').onchange = () => {
  display.classList.toggle('guest-mouse', $('mouse').checked);
  releaseMouse();
};
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
      setRunning(false, 'Guest stopped').catch(fail); fail(m.error); return;
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
  await rpc('init',{diagnostics:params.has('diagnostics'),manual:params.has('manual'),jit:params.get('jit')!=='0'});
  if (!(params.has('diagnostics') && params.get('renderer') === 'inline')) await startRenderer();
  // This client posts commands and caches metrics; Wasm exports stay in the Worker.
  core = new Proxy({}, {get:(_,name) => name === 'metric' ? id => metrics.get(id) :
    (...args) => rpc('call',{name,args})});
  await selectProfile();
  window.pc98 = {text:() => rpc('text'), read:addr => rpc('read',addr), stats:async () => ({...await rpc('stats'),rendererStartupMs,rendererFailure,renderStats:{...renderStats},presentationStats:{...presentationStats},presenterStats:presenter.diagnosticsState()}),
    exportBlob:async (drive=2) => {
      if (loading) throw Error('Media operation already in progress');
      loading = true; syncControls();
      try { return await exportBlob(drive); } finally { loading = false; syncControls(); }
    },
  };
  if (params.has('diagnostics')) window.pc98.audio = {
    suspend:async () => { await audio.suspend(); await soundState(); },
    resume:async () => { await audio.resume(); await soundState(); },
  };
  await defaultHdd();
} catch (error) { status('Unable to start'); fail(error); }
