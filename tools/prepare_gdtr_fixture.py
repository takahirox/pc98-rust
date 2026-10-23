#!/usr/bin/env python3
"""Build the original MIT Ap GDTR boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/gdtr.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/gdtr.asm -o gdtr.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/gdtr/gdtr.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'Reset readback, 24/32-bit loads, full-base stores with both operand sizes, exact bytes/guards, FFFA success, atomic restartable DS/SS #GP and register/LOCK #UD with prefixed saved IP checks and bounded handlers',
        'markers': ['GDTR PASS', 'GDTR FAIL'],
        'slot_budget': 2_000_000,
        'result_address': '0x8000',
        'result_length': 80,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/gdtr-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('GDTR fixture differs from fixtures/gdtr-manifest.json')
    destination = ROOT / 'fixtures/gdtr'
    destination.mkdir(exist_ok=True)
    (destination / 'gdtr.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'gdtr.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('GDTR fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
