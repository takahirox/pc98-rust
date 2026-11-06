#!/usr/bin/env python3
"""Build the real Rust core; use rustup's compiler and target consistently."""
import os
from pathlib import Path
import shutil
import subprocess
from build_media import ROOT

optimizer = ROOT / 'node_modules/.bin/wasm-opt'
if not optimizer.is_file():
    raise SystemExit('Missing pinned Wasm optimizer. Run npm ci before building the web core.')
subprocess.run(['python3', 'tools/prepare_desktop.py'], cwd=ROOT, check=True)
env = os.environ.copy()
if shutil.which('rustup') and not env.get('RUSTC'):
    env['RUSTC'] = subprocess.check_output(['rustup', 'which', 'rustc'], text=True).strip()
subprocess.run(['cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '--lib'], cwd=ROOT, env=env, check=True)
# Optimize the complete linked interpreter for host execution, preserving names
# for failure-only CPU profiles. npm's lockfile pins the same tool in CI/Pages.
# Guest instruction tables, scheduler clocks and PCM production are unchanged.
subprocess.run([str(optimizer), '-O3', '-g',
                str(ROOT / 'target/wasm32-unknown-unknown/release/pc98_rust.wasm'),
                '-o', str(ROOT / 'web/core.wasm')], cwd=ROOT, check=True)
print('Built web/core.wasm. Serve web/ with python3 -m http.server 8000 --directory web')
