#!/usr/bin/env python3
"""Check the opt-in block tier without replacing the published web/core.wasm."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
env = os.environ.copy()
env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -C target-feature=+simd128"
if shutil.which("rustup") and not env.get("RUSTC"):
    env["RUSTC"] = subprocess.check_output(["rustup", "which", "rustc"], text=True).strip()
subprocess.run(["cargo", "build", "--locked", "--release", "--target",
                "wasm32-unknown-unknown", "--lib", "--features", "block-experiment"],
               cwd=ROOT, env=env, check=True)
with tempfile.TemporaryDirectory(prefix="pc98-ram-blocks-") as directory:
    wasm = str(Path(directory) / "block.wasm")
    subprocess.run([str(ROOT / "node_modules/.bin/wasm-opt"), "-O3", "-g",
                    str(ROOT / "target/wasm32-unknown-unknown/release/pc98_rust.wasm"),
                    "-o", wasm], cwd=ROOT, check=True)
    env["PC98_BLOCK_WASM"] = wasm
    subprocess.run(["node", "--test", "tests/block-experiment.test.js"],
                   cwd=ROOT, env=env, check=True)
