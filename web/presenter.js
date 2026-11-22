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
  if (scan > 0.0 && density.y > 1.5) {
    float a = 3.14159265 / max(density.y,0.01);
    float attenuation = sin(a)/a * smoothstep(1.5,3.0,density.y);
    c *= 1.0 - scan * attenuation * (0.5-0.5*cos(6.2831853*(p.y-0.5)));
  }
  // One RGB triad per source pixel, only at >= 3 device pixels per triad.
  if (mask > 0.0 && density.x > 3.0 && density.y > 2.0) {
    float enabled = mask * smoothstep(3.0,4.0,density.x) * smoothstep(2.0,3.0,density.y);
    float m = 9.42477796 / max(density.x,0.01);
    c *= 1.0 - enabled * (0.5-0.5*cos(vec3(0,2.0943951,4.1887902)+p.x*6.2831853)) * sin(m)/m;
  }
  color = vec4(c,1);
}`;

// Software GL can synchronously read the output back during browser
// compositing. Shade at source resolution on a 2D canvas in that case.
function softwareRenderer(gl) {
  const info = gl.getExtension('WEBGL_debug_renderer_info');
  return info && /swiftshader|llvmpipe|softpipe|software rasterizer|basic render driver/i
    .test(gl.getParameter(info.UNMASKED_RENDERER_WEBGL));
}

export class Presenter {
  constructor(raw, output, surface, notify = () => {}, diagnostics = false) {
    this.raw = raw; this.output = output; this.surface = surface;
    this.context = raw.getContext('2d', {alpha:false});
    this.notify = text => { if (text !== this.message) { this.message = text; notify(text); } }; this.diagnostics = diagnostics;
    this.preset = defaultPreset; this.mask = false; this.hasFrame = false;
    this.stats = {uploads:0, uploadMs:0, submitMs:0, gpuMs:0, gpuSamples:0, failures:0, reusedFrames:0};
    this.queries = [];
    output.addEventListener('webglcontextlost', event => {
      if (event.target !== this.output) return;
      event.preventDefault(); this.gl = null; this.queries = []; this.timer = null;
      this.fallback('WebGL context lost');
    });
    output.addEventListener('webglcontextrestored', () => {
      if (output !== this.output) return;
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
    const gl = this.output.getContext('webgl2', {alpha:false, antialias:false, depth:false, stencil:false,
      preserveDrawingBuffer:false, failIfMajorPerformanceCaveat:true});
    if (!gl || softwareRenderer(gl)) {
      // A canvas that already owns a WebGL context can't acquire a 2D context.
      const original = this.output;
      const canvas = gl ? original.cloneNode(false) : original;
      const context = canvas.getContext('2d',{alpha:false});
      if (!context) throw Error('CRT presentation unavailable');
      if (gl) {
        original.replaceWith(canvas);
        this.output = canvas;
        gl.getExtension('WEBGL_lose_context')?.loseContext();
      }
      canvas.style.imageRendering = 'pixelated';
      canvas.addEventListener('contextlost', () => this.fallback('Canvas context lost'));
      canvas.addEventListener('contextrestored', () => {
        this.softwareFrame = null;
        this.failed = false;
        this.redraw();
      });
      this.software = context;
      return;
    }
    if (gl.isContextLost()) throw Error('WebGL2 unavailable');
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
      this.maxRenderbufferSize = gl.getParameter(gl.MAX_RENDERBUFFER_SIZE);
      const framebuffer = gl.createFramebuffer(), color = gl.createRenderbuffer();
      this.filtered = framebuffer && color ? {framebuffer,color,width:0,height:0,valid:false} : null;
      if (!this.filtered) {
        if (framebuffer) gl.deleteFramebuffer(framebuffer);
        if (color) gl.deleteRenderbuffer(color);
      }
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
  invalidate() { this.hasFrame = false; this.frameSource = null; }
  present(frame, source) {
    const previous = this.frameSource;
    const sameSize = this.raw.width === frame.width && this.raw.height === frame.height;
    // A row update depends on the renderer's preceding output. The DOM may
    // have discarded that frame, in which case the complete input is needed.
    const related = sameSize && this.hasFrame && source?.renderer && previous
      && source.renderer === previous.renderer && source.sequence > previous.sequence
      && source.generation === previous.generation && source.videoEpoch === previous.videoEpoch;
    const reuse = related && source.samePixelsAs === previous.sequence;
    this.frameSource = null;
    if (reuse) this.stats.reusedFrames++;
    else {
      if (!sameSize) { this.raw.width = frame.width; this.raw.height = frame.height; }
      const rows = related && source.changedRows?.after === previous.sequence ? source.changedRows : undefined;
      if (rows) this.context.putImageData(frame,0,0,0,rows.first,frame.width,rows.end-rows.first);
      else this.context.putImageData(frame,0,0);
      this.hasFrame = true;
      this.draw(frame.data,rows);
    }
    if (source?.renderer) this.frameSource = {renderer:source.renderer,sequence:source.sequence,
      generation:source.generation,videoEpoch:source.videoEpoch};
  }
  redraw() {
    if (!this.hasFrame) return;
    // Only UI/resize/restoration needs this copy. Normal frames upload their
    // incoming view directly and retain neither its buffer nor a second image.
    this.draw();
  }
  draw(bytes, rows) {
    if (this.preset === 'off') {
      this.output.hidden = true; this.surface.classList.remove('crt');
      this.notify('Original pixels · raw screenshots'); return;
    }
    if (this.failed) return;
    try {
      if (!this.gl && !this.software) this.initialize();
      this.surface.classList.add('crt');
      // Current PC-98 output is 640x400 with 1:1.2 pixels. Unknown future
      // dimensions default to square pixels rather than applying 1.2 twice.
      const aspect = this.raw.width === 640 && this.raw.height === 400 ? 4/3 : this.raw.width/this.raw.height;
      if (this.aspect !== aspect) {
        this.aspect = aspect;
        this.surface.style.setProperty('--crt-aspect',aspect);
        this.surface.style.setProperty('--crt-width-per-height',aspect);
      }
      if (this.software) {
        this.drawSoftware(bytes, rows);
        return;
      }
      const gl = this.gl;
      if (gl.isContextLost()) throw Error('WebGL context lost');
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
      } else if (rows) {
        gl.texSubImage2D(gl.TEXTURE_2D,0,0,rows.first,this.raw.width,rows.end-rows.first,
          gl.RGBA,gl.UNSIGNED_BYTE,pixels,rows.first*this.raw.width*4);
      } else gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,this.raw.width,this.raw.height,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
      const uploaded = performance.now();
      gl.uniform2f(this.uniforms.sourceSize,this.raw.width,this.raw.height);
      gl.uniform2f(this.uniforms.outputSize,width,height);
      gl.uniform1f(this.uniforms.scan,presets[this.preset].scan);
      gl.uniform1f(this.uniforms.softness,presets[this.preset].softness);
      gl.uniform1f(this.uniforms.mask,this.mask ? 0.025 : 0);
      this.drawCrt(width,height,allocate ? undefined : rows);
      if (query) {gl.endQuery(timer.TIME_ELAPSED_EXT); this.queries.push(query);}
      this.stats.uploads++; this.stats.uploadMs += uploaded-start; this.stats.submitMs += performance.now()-uploaded;
      if ((allocate || resized) && gl.getError() !== gl.NO_ERROR) throw Error('GPU allocation failed');
      this.output.hidden = false;
      this.notify('RGB CRT · 4:3 · screenshots of the screen include the filter');
    } catch (error) { this.stats.failures++; this.fallback(error.message); }
  }
  drawSoftware(bytes, rows) {
    const width = this.raw.width, height = this.raw.height;
    const started = performance.now();
    if (!this.softwareFrame || this.output.width !== width || this.output.height !== height) {
      this.output.width = width;
      this.output.height = height;
      this.softwareFrame = this.software.createImageData(width,height);
      rows = undefined;
    }
    const pixels = bytes || this.context.getImageData(0,0,width,height).data;
    const output = this.softwareFrame.data;
    const side = presets[this.preset].softness / 2, center = 1 - side * 2;
    const first = rows?.first ?? 0, end = rows?.end ?? height;
    for (let y = first; y < end; y++) {
      const start = y * width * 4, stop = start + width * 4;
      for (let at = start; at < stop; at += 4) {
        const left = at === start ? at : at - 4, right = at + 4 === stop ? at : at + 4;
        for (let channel = 0; channel < 3; channel++) {
          output[at+channel] = pixels[at+channel] * center + (pixels[left+channel] + pixels[right+channel]) * side;
        }
        output[at+3] = 255;
      }
    }
    this.software.putImageData(this.softwareFrame,0,0,0,first,width,end-first);
    this.stats.uploads++;
    this.stats.submitMs += performance.now()-started;
    this.output.hidden = false;
    this.notify('RGB CRT · lightweight · 4:3 · horizontal softness');
  }
  drawCrt(width,height,rows) {
    const gl = this.gl, target = this.filtered;
    const sourceHeight = this.raw.height;
    // Substantial changes are cheaper to shade directly than to shade and copy.
    if (!target || !rows || (rows.end-rows.first)*3 > sourceHeight
      || Math.max(width,height) > this.maxRenderbufferSize) {
      if (target) target.valid = false;
      gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
      return;
    }
    const resized = target.width !== width || target.height !== height;
    let clipped = false;
    gl.bindFramebuffer(gl.FRAMEBUFFER,target.framebuffer);
    try {
      if (resized) {
        gl.bindRenderbuffer(gl.RENDERBUFFER,target.color);
        gl.renderbufferStorage(gl.RENDERBUFFER,gl.RGBA8,width,height);
        gl.framebufferRenderbuffer(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.RENDERBUFFER,target.color);
        if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
          this.filtered = null;
          gl.bindFramebuffer(gl.FRAMEBUFFER,null);
          gl.deleteFramebuffer(target.framebuffer);
          gl.deleteRenderbuffer(target.color);
          gl.getError(); // Clear the failed offscreen allocation before the full draw.
          gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
          return;
        }
        target.width = width; target.height = height;
      }
      // The shader only filters horizontally. Round the affected source
      // rows outwards in the output viewport, whose Y axis starts at the bottom.
      if (target.valid && !resized) {
        const bottom = Math.floor((sourceHeight-rows.end)*height/sourceHeight);
        const top = Math.ceil((sourceHeight-rows.first)*height/sourceHeight);
        gl.enable(gl.SCISSOR_TEST);
        gl.scissor(0,bottom,width,top-bottom);
        clipped = true;
      }
      gl.drawArrays(gl.TRIANGLE_STRIP,0,4);
      if (clipped) { gl.disable(gl.SCISSOR_TEST); clipped = false; }
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER,null);
      gl.blitFramebuffer(0,0,width,height,0,0,width,height,gl.COLOR_BUFFER_BIT,gl.NEAREST);
      target.valid = true;
    } finally {
      if (clipped) gl.disable(gl.SCISSOR_TEST);
      gl.bindFramebuffer(gl.FRAMEBUFFER,null);
    }
  }
  diagnosticsState() {
    return {...this.stats, preset:this.preset, active:!this.output.hidden, fallback:Boolean(this.failed),
      backend:this.software ? 'canvas2d' : this.gl ? 'webgl2' : null,
      gpuTimerAvailable:Boolean(this.timer), width:this.output.width,height:this.output.height};
  }
}
