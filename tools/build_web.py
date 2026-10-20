#!/usr/bin/env python3
"""Build the real Rust core; use rustup's compiler and target consistently."""
import os
from pathlib import Path
import shutil
import subprocess
from build_media import ROOT

env = os.environ.copy()
if shutil.which('rustup') and not env.get('RUSTC'):
    env['RUSTC'] = subprocess.check_output(['rustup', 'which', 'rustc'], text=True).strip()
subprocess.run(['cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '--lib'], cwd=ROOT, env=env, check=True)
shutil.copyfile(ROOT / 'target/wasm32-unknown-unknown/release/pc98_rust.wasm', ROOT / 'web/core.wasm')
print('Built web/core.wasm. Serve web/ with python3 -m http.server 8000 --directory web')
