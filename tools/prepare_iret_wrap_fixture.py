#!/usr/bin/env python3
"""Build the original MIT Ap IRET wrap boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/iret_wrap.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/iret_wrap.asm -o iret_wrap.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/iret_wrap/iret_wrap.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'Real-mode BIOS interrupt/IRET at SP=2 plus two word and two dword return frames; SP wraps between operands, flags and guards checked by guest',
        'markers': ['IRET WRAP PASS', 'IRET WRAP FAIL'],
        'slot_budget': 100_000,
        'result_address': '0x8000',
        'result_length': 3,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/iret-wrap-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('IRET wrap fixture differs from fixtures/iret-wrap-manifest.json')
    destination = ROOT / 'fixtures/iret_wrap'
    destination.mkdir(exist_ok=True)
    (destination / 'iret_wrap.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'iret_wrap.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('IRET wrap fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
