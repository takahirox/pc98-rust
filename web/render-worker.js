// Render-only Wasm instance: never calls Machine, guest steps, devices or audio.
// One full snapshot and one RGBA buffer circulate; a slow DOM applies backpressure.
let core, port, snapshot, pixels = new ArrayBuffer(640*400*4);
let lastSequence = 0, held = false, delayed;
let totals = {frames:0, rasterMs:0, copyMs:0, transportMs:0};
onmessage = async ({data:m}) => {
  try {
    if (m.type === 'init') {
      const response = await fetch(new URL('core.wasm', import.meta.url));
      if (!response.ok) throw Error(`Render Wasm HTTP ${response.status}`);
      core = (await WebAssembly.instantiate(await response.arrayBuffer(),{})).instance.exports;
      if (m.diagnostics) globalThis.pc98Render = {
        hold:() => { held = true; },
        pending:() => Number(!!delayed),
        release:() => { held = false; if (delayed) { const frame = delayed; delayed = null; renderFrame(frame); } },
      };
      postMessage({event:'ready'});
    } else if (m.type === 'connect') {
      port = m.port;
      port.onmessage = ({data:frame}) => {
        if (held) {
          if (delayed) { postMessage({event:'fault',error:'Overlapping held snapshot'}); return; }
          delayed = frame; return;
        }
        renderFrame(frame);
      };
      port.onmessageerror = () => postMessage({event:'fault',error:'Invalid snapshot message'});
      port.start();
    } else if (m.type === 'frameAck') {
      if (pixels || !snapshot || !(m.buffer instanceof ArrayBuffer) || m.sequence !== snapshot.sequence || m.buffer.byteLength !== 640*400*4) {
        throw Error('Invalid rendered frame credit');
      }
      pixels = m.buffer;
      const frame = snapshot; snapshot = null;
      port.postMessage({buffer:frame.buffer,sequence:frame.sequence,captureAt:m.captureAt},[frame.buffer]);
    }
  } catch (error) { postMessage({event:'fault',error:error.message}); }
};

function renderFrame(frame) {
  try {
    if (snapshot || !pixels || !Number.isSafeInteger(frame.sequence) || frame.sequence <= lastSequence
        || !(frame.buffer instanceof ArrayBuffer) || ![219952,547632].includes(frame.length) || frame.length > frame.buffer.byteLength
        || !Number.isSafeInteger(frame.videoEpoch) || !Number.isSafeInteger(frame.generation)) {
      throw Error('Invalid render snapshot ownership or sequence');
    }
    const clock = new DataView(frame.buffer);
    if (clock.getUint32(8,true)+clock.getUint32(12,true)*4294967296 !== frame.guestCycles) {
      throw Error('Snapshot guest-clock stamp mismatch');
    }
    lastSequence = frame.sequence;
    snapshot = frame;
    const began = performance.now();
    const ptr = core.input_buffer(frame.length) >>> 0;
    if (!ptr) throw Error('Unable to allocate render snapshot');
    new Uint8Array(core.memory.buffer,ptr,frame.length).set(new Uint8Array(frame.buffer,0,frame.length));
    const copied = performance.now();
    const output = core.video_render(frame.length) >>> 0;
    if (!output) throw Error('Invalid video snapshot');
    const rasterized = performance.now();
    new Uint8Array(pixels).set(new Uint8Array(core.memory.buffer,output,640*400*4));
    totals.wasmBytes = core.memory.buffer.byteLength;
    totals.frames++;
    totals.rasterMs += rasterized-copied;
    totals.copyMs += copied-began+performance.now()-rasterized;
    totals.transportMs += performance.timeOrigin+began-frame.captured;
    postMessage({...frame,event:'frame',buffer:pixels,renderStats:{...totals}},[pixels]);
    pixels = null;
  } catch (error) { postMessage({event:'fault',error:error.message}); }
}
