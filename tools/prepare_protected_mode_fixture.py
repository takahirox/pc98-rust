#!/usr/bin/env python3
"""Build the wholly original MIT paging-disabled protected-mode guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/protected_mode.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568-len(boot))
    result = bytearray([0xa5]*64)
    result[1:5] = bytes([0x16, 1, 1, 1])
    result[9:13] = bytes([0x32]*4)
    result[17] = 0x67
    result[25:29] = bytes([0x55, 0x4e, 0x46, 0xa5])
    result[33] = 0x52
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/protected_mode.asm -o protected_mode.bin',
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/protected_mode/protected_mode.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Real -> PE 16-bit -> PE 32-bit -> PE 16-bit -> real; GDT/LDT, all six segments, EIP/ESP above 64 KiB, 66/67, near/far CALL/RET, IDT INT/GP repair/IRETD, PC-98 A20 and mapped RAM, PIC/PIT IRQ0 IF gating/ISR/EOI, guards, cold/reset',
        'result_address': '0x8000', 'result_length': 64,
        'result_hex': result.hex(),
        'extended_address': '0x210001', 'extended_hex': 'cdab3412',
        'markers': ['PROTECTED MODE PASS', 'PROTECTED MODE FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/protected-mode-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('Protected-mode fixture differs from checked manifest')
    dest = ROOT/'fixtures/protected_mode'
    dest.mkdir(exist_ok=True)
    (dest/'protected_mode.hdm').write_bytes(image)
    (dest/'protected_mode.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('Protected-mode fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
