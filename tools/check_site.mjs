// Validate exactly the web/ tree that upload-pages-artifact will publish.
import assert from 'node:assert/strict';
import { readdir, readFile } from 'node:fs/promises';

const site = new URL('../web/', import.meta.url);
const expected = [
  'index.html', 'app.js', 'style.css', 'core.wasm',
  'licenses.html', 'LICENSE.txt', 'hello98-LICENSE.txt', 'shinonome-LICENSE.txt',
  'media/hello98.tfd', 'media/startrail.hdm',
].sort();
async function files(directory, prefix = '') {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const name = prefix + entry.name;
    if (entry.isDirectory()) {
      assert.equal(name, 'media', `Unexpected publication directory: ${name}`);
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
const wasm = await readFile(new URL('core.wasm', site));
assert.deepEqual([...wasm.subarray(0, 8)], [0, 97, 115, 109, 1, 0, 0, 0]);
// Compile the entire module; a magic header alone is not a valid Wasm binary.
const module = await WebAssembly.compile(wasm);
const exported = WebAssembly.Module.exports(module).map(entry => entry.name);
for (const name of ['memory', 'insert', 'step', 'frame', 'key', 'reset', 'set_profile']) {
  assert(exported.includes(name), `Missing emulator export: ${name}`);
}
console.log(`Publication checked: ${expected.length} approved assets; valid emulator Wasm (${wasm.length} bytes)`);
