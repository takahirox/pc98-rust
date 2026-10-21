#!/usr/bin/env python3
"""Reproduce our MIT guest/firmware and cleared third-party boot disk, offline."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent.parent

def assemble(source):
    with tempfile.TemporaryDirectory() as td:
        out = Path(td) / 'guest.bin'
        subprocess.run(['nasm', '-f', 'bin', str(ROOT / source), '-o', str(out)], check=True)
        return out.read_bytes()

def products():
    game = assemble('guest/game.asm')
    assert len(game) == 8192
    hello = (ROOT / 'fixtures/hello98/boot.bin').read_bytes()
    assert hashlib.sha256(hello).hexdigest() == 'bdf20204b963227b826623f2a16a43f9d9cf8e0d4f1fef337b9fbd2276606cda'
    return {
        'guest/firmware.bin': assemble('guest/firmware.asm'),
        'web/LICENSE.txt': (ROOT / 'LICENSE').read_bytes(),
        'web/hello98-LICENSE.txt': (ROOT / 'fixtures/hello98/LICENSE').read_bytes(),
        'web/shinonome-LICENSE.txt': (ROOT / 'fixtures/fonts/LICENSE').read_bytes() + b'\n' + (ROOT / 'fixtures/fonts/AUTHORS').read_bytes(),
        'web/media/startrail.hdm': game + bytes(1261568 - len(game)),
        'web/media/hello98.tfd': hello + bytes(1261568 - len(hello)),
    }

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    args = p.parse_args()
    for name, data in products().items():
        dest = ROOT / name
        if args.check:
            if not dest.exists() or dest.read_bytes() != data:
                raise SystemExit(f'{name} is stale; run python3 tools/build_media.py')
        else:
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
        print(f'{name}: {hashlib.sha256(data).hexdigest()}')
