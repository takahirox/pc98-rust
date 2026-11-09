// First-party PCM delivery only. Guest synthesis and clocks stay in the Worker.
class PC98Audio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = []; this.offset = 0; this.phase = 0; this.epoch = 0;
    this.energy = 0; this.samples = 0; this.underflows = 0; this.overflows = 0;
    this.receivedSamples = 0; this.consumedSamples = 0; this.queuedSamples = 0;
    this.sequenceErrors = 0; this.staleBlocks = 0; this.checksumErrors = 0; this.checkedBlocks = 0;
    this.prebufferSamples = 2400; // 50 ms, inside the six transferable credits.
    this.waiting = true; this.started = false;
    this.underflowRun = 0; this.maxUnderflowRun = 0; this.reportFrames = 0;
    this.port.onmessage = ({data:m}) => {
      if (m.type === 'connect') {
        this.input = m.port;
        this.input.onmessage = ({data}) => this.receive(data);
        this.input.start();
      }
    };
  }
  report(type, buffer) {
    this.input.postMessage({type, epoch:this.epoch, buffer, stats:{
      audioEnergy:this.energy, scheduledSamples:this.samples, queuedBlocks:this.queue.length,
      underflows:this.underflows, overflows:this.overflows, audioEpoch:this.epoch,
      receivedSamples:this.receivedSamples, consumedSamples:this.consumedSamples,
      queuedSamples:this.queuedSamples, sequenceErrors:this.sequenceErrors,
      staleBlocks:this.staleBlocks, underflowRun:this.underflowRun, maxUnderflowRun:this.maxUnderflowRun,
      checksumErrors:this.checksumErrors, checkedBlocks:this.checkedBlocks,
      deviceSampleRate:sampleRate,
    }}, buffer ? [buffer] : []);
    this.reportFrames = 0;
  }
  release(block) { this.report('recycle',block.buffer); }
  receive(m) {
    if (m.type === 'flush') {
      this.epoch = m.epoch;
      this.receivedSamples = this.consumedSamples = this.queuedSamples = 0;
      this.waiting = true; this.started = false; this.underflowRun = 0;
      while (this.queue.length) this.release(this.queue.shift());
      this.offset = 0; this.phase = 0;
      this.report('flushed');
    } else if (m.type === 'pcm') {
      if (m.epoch !== this.epoch) { this.staleBlocks++; this.release(m); return; }
      if (this.queue.length >= 6) { this.overflows++; this.release(m); return; }
      if (m.start !== this.receivedSamples) this.sequenceErrors++;
      this.receivedSamples = m.start + m.count;
      this.queuedSamples += m.count;
      this.queue.push({...m, samples:new Float32Array(m.buffer),
        bits:m.checksum === undefined ? null : new Uint32Array(m.buffer), hash:2166136261});
    }
  }
  advance() {
    // Retain fractional phase and any downsampling skip across block edges.
    while (this.phase >= sampleRate && this.queue.length) {
      const block = this.queue[0];
      if (block.bits) block.hash = Math.imul(block.hash ^ block.bits[this.offset],16777619) >>> 0;
      this.phase -= sampleRate; this.consumedSamples++; this.queuedSamples--;
      if (++this.offset >= block.count) {
        if (block.bits) { this.checkedBlocks++; if (block.hash !== block.checksum) this.checksumErrors++; }
        this.offset = 0; this.release(this.queue.shift());
      }
    }
  }
  process(_inputs, outputs) {
    const output = outputs[0][0];
    if (!output) return true;
    for (let i = 0; i < output.length; i++) {
      if (this.waiting && this.queuedSamples >= this.prebufferSamples) {
        this.waiting = false; this.started = true;
      }
      this.advance();
      const block = this.queue[0];
      if (this.waiting || !block) {
        output[i] = 0; this.waiting = true;
        if (this.started) {
          this.underflows++; this.underflowRun++;
          this.maxUnderflowRun = Math.max(this.maxUnderflowRun,this.underflowRun);
          // Credit reports can hide gaps shorter than the periodic interval.
          if (this.input && this.underflowRun === 1) this.report('stats');
        }
        continue;
      }
      const recovered = this.underflowRun !== 0;
      this.underflowRun = 0;
      // End preview permission immediately when real PCM resumes.
      if (this.input && recovered) this.report('stats');
      const value = block.samples[this.offset];
      output[i] = value; this.energy += value*value; this.samples++;
      // Integer phase avoids accumulated rounding drift at 44.1 kHz.
      this.phase += 48000; this.advance();
    }
    for (let ch = 1; ch < outputs[0].length; ch++) outputs[0][ch].set(output);
    // A starved sink still reports, so diagnostics cannot hide a persistent
    // dropout behind the last successful recycle. No unbounded event history.
    this.reportFrames += output.length;
    if (this.input && this.reportFrames >= sampleRate/4) this.report('stats');
    return true;
  }
}
registerProcessor('pc98-audio', PC98Audio);
