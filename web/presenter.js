// Host-only RGB monitor presentation. The 2D canvas remains the raw capture
// surface and always contains the newest frame before a transferable is acked.
export const defaultPreset = 'sharp';
export const presets = Object.freeze({
  off: {scan:0, softness:0},
  sharp: {scan:0.06, softness:0.035},
  soft: {scan:0.08, softness:0.09},
  strong: {scan:0.11, softness:0.07},
});
const vertex = `#version 300 es
in vec2 position;
out vec2 uv;
void main() { uv = (position + 1.0) * 0.5; gl_Position = vec4(position,0,1); }`;
const fragment = `#version 300 es
precision highp float;
uniform sampler2D image;
uniform vec2 sourceSize, outputSize;
uniform float scan, softness, mask;
in vec2 uv;
out vec4 color;
void main() {
  vec2 p = vec2(uv.x,1.0-uv.y) * sourceSize;
  vec2 center = (floor(p)+0.5)/sourceSize;
  vec3 c = texture(image,center).rgb;
  vec2 dx = vec2(1.0/sourceSize.x,0.0);
  // A small horizontal three-tap spot; no vertical blur of Japanese glyphs.
  c = mix(c,0.5*(texture(image,center-dx).rgb+texture(image,center+dx).rgb),softness);
  vec2 density = outputSize/sourceSize;
  // Fade modulation below two output pixels per source line, and integrate
  // the sinusoid across the output pixel footprint to suppress scale moire.
  float a = 3.14159265 / max(density.y,0.01);
  float attenuation = sin(a)/a * smoothstep(1.5,3.0,density.y);
  c *= 1.0 - scan * attenuation * (0.5-0.5*cos(6.2831853*(p.y-0.5)));
  // One RGB triad per source pixel, only at >= 3 device pixels per triad.
  float enabled = mask * smoothstep(3.0,4.0,density.x) * smoothstep(2.0,3.0,density.y);
  float m = 9.42477796 / max(density.x,0.01);
  c *= 1.0 - enabled * (0.5-0.5*cos(vec3(0,2.0943951,4.1887902)+p.x*6.2831853)) * sin(m)/m;
  color = vec4(c,1);
}`;

export class Presenter {
  constructor(raw, output, surface, notify = () => {}, diagnostics = false) {
    this.raw = raw; this.output = output; this.surface = surface;
    this.context = raw.getContext('2d', {alpha:false});
    this.notify = text => { if (text !== this.message) { this.message = text; notify(text); } }; this.diagnostics = diagnostics;
    this.preset = defaultPreset; this.mask = false; this.hasFrame = false;
    this.stats = {uploads:0, uploadMs:0, submitMs:0, gpuMs:0, gpuSamples:0, failures:0};
    this.queries = [];
    output.addEventListener('webglcontextlost', event => {
      event.preventDefault(); this.gl = null; this.queries = []; this.timer = null;
      this.fallback('WebGL context lost');
    });
    output.addEventListener('webglcontextrestored', () => {
      this.failed = false; this.redraw();
    });
    this.observer = new ResizeObserver(() => this.redraw());
    this.observer.observe(surface);
    window.addEventListener('resize', () => this.redraw());
    // Also observe monitor/DPR changes that do not change CSS dimensions.
    const watchDpr = () => {
      const query = matchMedia(`(resolution: ${devicePixelRatio}dppx)`);
      query.addEventListener('change', () => { this.redraw(); watchDpr(); }, {once:true});
    };
    watchDpr();
  }
  fallback(reason) {
    this.failed = true; this.output.hidden = true;
    this.surface.classList.remove('crt');
    this.notify(`${reason}; showing original pixels`);
  }
  initialize() {
    const gl = this.output.getContext('webgl2', {alpha:false, antialias:false, depth:false, stencil:false, preserveDrawingBuffer:false});
    if (!gl || gl.isContextLost()) throw Error('WebGL2 unavailable');
    this.gl = gl;
    const shaders = [];
    let program, buffer, texture;
    try {
      for (const [type, source] of [[gl.VERTEX_SHADER,vertex],[gl.FRAGMENT_SHADER,fragment]]) {
        const shader = gl.createShader(type); shaders.push(shader);
        gl.shaderSource(shader,source); gl.compileShader(shader);
        if (!gl.getShaderParameter(shader,gl.COMPILE_STATUS)) throw Error('CRT shader compilation failed');
      }
      program = gl.createProgram();
      for (const shader of shaders) gl.attachShader(program,shader);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program,gl.LINK_STATUS)) throw Error('CRT shader linking failed');
      gl.useProgram(program);
      buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER,buffer);
      gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,1,-1,-1,1,1,1]),gl.STATIC_DRAW);
      const attribute = gl.getAttribLocation(program,'position');
      gl.enableVertexAttribArray(attribute); gl.vertexAttribPointer(attribute,2,gl.FLOAT,false,0,0);
      texture = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D,texture);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);
      this.uniforms = Object.fromEntries(['image','sourceSize','outputSize','scan','softness','mask'].map(n=>[n,gl.getUniformLocation(program,n)]));
      gl.uniform1i(this.uniforms.image,0);
      this.textureWidth = 0; this.textureHeight = 0;
      this.timer = this.diagnostics ? gl.getExtension('EXT_disjoint_timer_query_webgl2') : null;
      this.maxTextureSize = gl.getParameter(gl.MAX_TEXTURE_SIZE);
      this.failed = false;
    } catch (error) {
      if (texture) gl.deleteTexture(texture);
      if (buffer) gl.deleteBuffer(buffer);
      if (program) gl.deleteProgram(program);
      throw error;
    } finally { for (const shader of shaders) gl.deleteShader(shader); }
  }
  setPreset(value, mask = this.mask) {
    this.preset = typeof value === 'string' && Object.hasOwn(presets,value) ? value : defaultPreset; this.mask = Boolean(mask);
    this.redraw();
  }
  present(frame) {
    if (this.raw.width !== frame.width || this.raw.height !== frame.height) {
      this.raw.width = frame.width; this.raw.height = frame.height;
    }
    this.context.putImageData(frame,0,0); this.hasFrame = true;
    this.draw(frame.data);
  }
  redraw() {
    if (!this.hasFrame) return;
    // Only UI/resize/restoration needs this copy. Normal frames upload their
    // incoming view directly and retain neither its buffer nor a second image.
    this.draw();
  }
  draw(bytes) {
    if (this.preset === 'off') {
      this.output.hidden = true; this.surface.classList.remove('crt');
      this.notify('Original pixels · raw screenshots'); return;
    }
    if (this.failed) return;
    try {
      if (!this.gl) this.initialize();
      const gl = this.gl;
      if (gl.isContextLost()) throw Error('WebGL context lost');
      this.surface.classList.add('crt');
      // Current PC-98 output is 640x400 with 1:1.2 pixels. Unknown future
      // dimensions default to square pixels rather than applying 1.2 twice.
      const aspect = this.raw.width === 640 && this.raw.height === 400 ? 4/3 : this.raw.width/this.raw.height;
      if (this.aspect !== aspect) {
        this.aspect = aspect;
        this.surface.style.setProperty('--crt-aspect',aspect);
        this.surface.style.setProperty('--crt-width-per-height',aspect);
      }
      const bounds = this.surface.getBoundingClientRect();
      const scale = Math.min(devicePixelRatio || 1,2,2560/Math.max(1,bounds.width),1920/Math.max(1,bounds.height),
        this.maxTextureSize/Math.max(1,bounds.width,bounds.height));
      const width = Math.max(1,Math.round(bounds.width*scale)), height = Math.max(1,Math.round(bounds.height*scale));
      const resized = this.output.width !== width || this.output.height !== height;
      if (resized) {this.output.width = width; this.output.height = height;}
      gl.viewport(0,0,width,height);
      const timer = this.timer;
      if (timer) {
        const disjoint = gl.getParameter(timer.GPU_DISJOINT_EXT);
        while (this.queries.length && (disjoint || gl.getQueryParameter(this.queries[0],gl.QUERY_RESULT_AVAILABLE))) {
          const query = this.queries.shift();
          if (!disjoint) {this.stats.gpuMs += gl.getQueryParameter(query,gl.QUERY_RESULT)/1e6; this.stats.gpuSamples++;}
          gl.deleteQuery(query);
        }
      }
      const query = timer && this.queries.length < 4 ? gl.createQuery() : null;
      if (query) gl.beginQuery(timer.TIME_ELAPSED_EXT,query);
      const start = performance.now();
      const pixels = bytes || this.context.getImageData(0,0,this.raw.width,this.raw.height).data;
      const allocate = this.textureWidth !== this.raw.width || this.textureHeight !== this.raw.height;
      if (allocate) {
        gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,this.raw.width,this.raw.height,0,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
        this.textureWidth = this.raw.width; this.textureHeight = this.raw.height;
      } else gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,this.raw.width,this.raw.height,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
      const uploaded = performance.now();
      gl.uniform2f(this.uniforms.sourceSize,this.raw.width,this.raw.height);
      gl.uniform2f(this.uniforms.outputSize,width,height);
      gl.uniform1f(this.uniforms.scan,presets[this.preset].scan);
      gl.uniform1f(this.uniforms.softness,presets[this.preset].softness);
      gl.uniform1f(this.uniforms.mask,this.mask ? 0.025 : 0);
      gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
      if (query) {gl.endQuery(timer.TIME_ELAPSED_EXT); this.queries.push(query);}
      this.stats.uploads++; this.stats.uploadMs += uploaded-start; this.stats.submitMs += performance.now()-uploaded;
      if ((allocate || resized) && gl.getError() !== gl.NO_ERROR) throw Error('GPU allocation failed');
      this.output.hidden = false;
      this.notify('RGB CRT · 4:3 · screenshots of the screen include the filter');
    } catch (error) { this.stats.failures++; this.fallback(error.message); }
  }
  diagnosticsState() {
    return {...this.stats, preset:this.preset, active:!this.output.hidden, fallback:Boolean(this.failed),
      gpuTimerAvailable:Boolean(this.timer), width:this.output.width,height:this.output.height};
  }
}
