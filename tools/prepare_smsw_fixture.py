#!/usr/bin/env python3
"""Build the original MIT Ap SMSW boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/smsw.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytearray([0xa5] * 80)
    for offset, value in [(1, 0xabcd0010), (9, 0xbcde0010), (65, 0xcdef0010)]:
        result[offset:offset+4] = value.to_bytes(4, 'little')
    for offset in [17, 25, 33, 41, 49, 57]:
        result[offset:offset+2] = bytes.fromhex('1000')
    result[73:75] = bytes([2, 2])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/smsw.asm -o smsw.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/smsw/smsw.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'Reset MSW 0010h, preserved register upper halves and flags, exact two-byte memory stores with either operand size, repeated size overrides, SS SIB, FFFE, atomic DS/SS #GP and LOCK register/memory #UD with six-byte first-prefix frames, guest repair/IRET/retry, Machine reset',
        'result_address': '0x8000',
        'result_length': 80,
        'result_hex': result.hex(),
        'last_operand_address': '0xfffd',
        'last_operand_hex': 'a51000',
        'markers': ['SMSW PASS', 'SMSW FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/smsw-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('SMSW fixture differs from fixtures/smsw-manifest.json')
    destination = ROOT / 'fixtures/smsw'
    destination.mkdir(exist_ok=True)
    (destination / 'smsw.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'smsw.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('SMSW fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
