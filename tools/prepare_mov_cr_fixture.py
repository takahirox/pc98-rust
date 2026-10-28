#!/usr/bin/env python3
"""Build the original MIT Ap MOV CR boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/mov_cr.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytearray([0xa5] * 128)
    for offset in range(1, 64, 8):
        result[offset:offset+4] = bytes.fromhex('10000060')
    for offset in [65, 73]:
        result[offset:offset+4] = bytes(4)
    result[81:83] = bytes.fromhex('1000')
    result[89:91] = bytes.fromhex('d708')
    result[97] = 3
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/mov_cr.asm -o mov_cr.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/mov_cr/mov_cr.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'i486 CR0 60000010h to all eight full GPRs including ESP; CR2/CR3 zero reset policy; SMSW 0010h; preserved FLAGS; size/segment/REP prefixes; LOCK, CR4 and memory encoding #UD; exact six-byte first-prefix frames; guest repair/IRET/retry; cold boot and Machine reset',
        'result_address': '0x8000',
        'result_length': 128,
        'result_hex': result.hex(),
        'markers': ['MOV CR PASS', 'MOV CR FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/mov-cr-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('MOV CR fixture differs from fixtures/mov-cr-manifest.json')
    destination = ROOT / 'fixtures/mov_cr'
    destination.mkdir(exist_ok=True)
    (destination / 'mov_cr.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'mov_cr.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('MOV CR fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
