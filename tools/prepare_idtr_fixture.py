#!/usr/bin/env python3
"""Build the original MIT Ap IDTR boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/idtr.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytearray([0xa5] * 80)
    result[1:7] = bytes.fromhex('ff0300000000')
    for offset in [9, 17, 49]:
        result[offset:offset+6] = bytes.fromhex('ff0378563412')
    result[25:31] = bytes.fromhex('ff0378563400')
    for offset in [33, 73]:
        result[offset:offset+6] = bytes.fromhex('c30001600000')
    result[41:44] = bytes([3, 2, 2])
    result[57] = 0x88
    result[65] = 0x77
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/idtr.asm -o idtr.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/idtr/idtr.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'Architectural reset, 24/32-bit loads, full-base stores, address32, unaligned relocated vectors versus conflicting fixed IVT, INT/INT3/INTO, FFFA, restartable DS/SS #GP, register/LOCK #UD, inclusive limit #DF with six-byte frame, wrapping 32-bit linear base arithmetic, terminal limit-22h shutdown and Machine reset',
        'result_hex': result.hex(),
        'last_operand_address': '0xfff9',
        'last_operand_hex': 'a5c30001600000',
        'terminal_shutdown': True,
        'markers': ['IDTR PASS', 'IDTR FAIL'],
        'slot_budget': 2_000_000,
        'result_address': '0x8000',
        'result_length': 80,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/idtr-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('IDTR fixture differs from fixtures/idtr-manifest.json')
    destination = ROOT / 'fixtures/idtr'
    destination.mkdir(exist_ok=True)
    (destination / 'idtr.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'idtr.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('IDTR fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
