// Validate exactly the web/ tree that upload-pages-artifact will publish.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readdir, readFile } from 'node:fs/promises';

const site = new URL('../web/', import.meta.url);
const desktop = JSON.parse(await readFile(new URL('../fixtures/desktop-publication.json', import.meta.url)));
const expected = [
  ...Object.keys(desktop).map(name => `freedos/${name}`),
  'index.html', 'app.js', 'emulator-worker.js', 'render-worker.js', 'audio-worklet.js', 'style.css', 'core.wasm',
  'licenses.html', 'LICENSE.txt', 'hello98-LICENSE.txt', 'shinonome-LICENSE.txt',
  'media/hello98.tfd', 'media/startrail.hdm',
].sort();
async function files(directory, prefix = '') {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const name = prefix + entry.name;
    if (entry.isDirectory()) {
      assert(['media', 'freedos'].includes(name), `Unexpected publication directory: ${name}`);
      result.push(...await files(new URL(`${entry.name}/`, directory), `${name}/`));
    } else {
      assert(entry.isFile(), `Publication must contain only regular files: ${name}`);
      assert((await readFile(new URL(entry.name, directory))).length > 0, `Empty asset: ${name}`);
      result.push(name);
    }
  }
  return result;
}
assert.deepEqual((await files(site)).sort(), expected, 'Unexpected or missing publication assets');
for (const [name, digest] of Object.entries(desktop)) {
  const bytes = await readFile(new URL(`freedos/${name}`, site));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), digest, `Changed guest/source asset: ${name}`);
}
const wasm = await readFile(new URL('core.wasm', site));
assert.deepEqual([...wasm.subarray(0, 8)], [0, 97, 115, 109, 1, 0, 0, 0]);
// Compile the entire module; a magic header alone is not a valid Wasm binary.
const module = await WebAssembly.compile(wasm);
const exported = WebAssembly.Module.exports(module).map(entry => entry.name);
for (const name of ['memory', 'insert', 'step', 'frame', 'key', 'mouse', 'reset', 'set_profile', 'export_disk', 'set_boot_source', 'video_snapshot', 'video_snapshot_len', 'video_render']) {
  assert(exported.includes(name), `Missing emulator export: ${name}`);
}
console.log(`Publication checked: ${expected.length} approved assets; valid emulator Wasm (${wasm.length} bytes)`);
// Every execution entry point is a local, approved asset resolved relative to
// its module, so the same artifact works at / and /pc98-rust/.
const app = await readFile(new URL('app.js',site),'utf8');
const worker = await readFile(new URL('emulator-worker.js',site),'utf8');
for (const entry of ['emulator-worker.js','render-worker.js','audio-worklet.js']) {
  assert(app.includes(`new URL('${entry}',import.meta.url)`), `Missing relative entry point: ${entry}`);
}
assert(worker.includes("new URL('core.wasm', import.meta.url)"));
assert(!/WebAssembly\.|core\.(step|frame|memory)|createBufferSource/.test(app), 'DOM must not execute Wasm or batch audio');

const renderer = await readFile(new URL('render-worker.js',site),'utf8');
assert(renderer.includes("new URL('core.wasm', import.meta.url)"));
assert(!/core\.(step|frame|metric|audio|reset|set_profile)/.test(renderer), 'Renderer must not own a guest Machine');
