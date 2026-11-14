// Opt-in owner for the block-experiment Wasm build. Production keeps WasmJit.
import {WasmJit} from '../../web/jit.js';
import {compileRamBlock} from './block-compiler.mjs';
export class BlockJit extends WasmJit {
  constructor(core,options={}){
    if(typeof core.block_begin_slice!=='function')throw Error('Build with --features block-experiment');
    core.jit_clear();
    super({...core,jit_begin_slice:core.block_begin_slice},options);
    this.native=core;
  }
  stats(){return {...super.stats(),mode:this.failed?'interpreter':'block-experiment',blockCalls:this.native.block_metric(0),blockInstructions:this.native.block_metric(1),blockFallbacks:this.native.block_metric(2)};}
  offer(id){
    const core=this.native,meta=new Uint32Array(core.memory.buffer,core.block_packet()>>>0,core.block_packet_len()).slice();
    if(!meta[0])return super.offer(id);
    const packet=new Uint32Array(core.memory.buffer,core.jit_packet()>>>0,core.jit_packet_len()).slice();
    if(packet[0]!==id)throw Error('Experimental plan identity mismatch');
    const token={cancelled:false};this.pending=token;const started=performance.now();
    Promise.resolve().then(()=>{
      if(token.cancelled)return null;
      const bytes=compileRamBlock(packet,meta);this.bytes+=bytes.length;return this.compile(bytes);
    }).then(module=>{
      if(this.pending!==token||token.cancelled)return;
      const instance=new WebAssembly.Instance(module,{core:{memory:core.memory,finish:core.block_finish}});
      if(!core.jit_register(id,1))return;
      if(this.cache.size>=4096){const old=this.cache.keys().next().value;this.cache.delete(old);core.jit_register(old,0);}
      this.table.set(id&4095,instance.exports.run);this.cache.set(id,instance.exports.run);this.compiled++;
    }).catch(error=>{
      if(this.pending===token&&!token.cancelled){this.compileError=String(error);core.jit_register(id,0);this.failed=true;this.failures++;}
    }).finally(()=>{
      if(this.pending===token){if(!token.cancelled)this.compilationMs+=performance.now()-started;this.pending=null;}
    });
  }
}
