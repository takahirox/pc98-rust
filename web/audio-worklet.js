// First-party PCM delivery only. Guest synthesis and clocks stay in the Worker.
class PC98Audio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = []; this.offset = 0; this.phase = 0; this.epoch = 0;
    this.energy = 0; this.samples = 0; this.underflows = 0; this.overflows = 0;
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
    }}, buffer ? [buffer] : []);
  }
  release(block) { this.report('recycle',block.buffer); }
  receive(m) {
    if (m.type === 'flush') {
      this.epoch = m.epoch;
      while (this.queue.length) this.release(this.queue.shift());
      this.offset = 0; this.phase = 0;
      this.report('flushed');
    } else if (m.type === 'pcm') {
      if (m.epoch !== this.epoch || this.queue.length >= 6) {
        this.overflows++; this.release(m); return;
      }
      this.queue.push({...m, samples:new Float32Array(m.buffer)});
    }
  }
  process(_inputs, outputs) {
    const output = outputs[0][0];
    if (!output) return true;
    // Consume 48 kHz source samples even when the physical device runs at a
    // different rate. Fractional phase is retained across blocks and quanta.
    const ratio = 48000 / sampleRate;
    for (let i = 0; i < output.length; i++) {
      const block = this.queue[0];
      if (!block) { output[i] = 0; this.underflows++; continue; }
      const value = block.samples[this.offset];
      output[i] = value; this.energy += value*value; this.samples++;
      this.phase += ratio;
      while (this.phase >= 1 && this.queue.length) {
        this.phase--;
        if (++this.offset >= this.queue[0].count) { this.release(this.queue.shift()); this.offset = 0; }
      }
      if (!this.queue.length) this.phase = 0;
    }
    for (let ch = 1; ch < outputs[0].length; ch++) outputs[0][ch].set(output);
    return true;
  }
}
registerProcessor('pc98-audio', PC98Audio);
