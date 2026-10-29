#!/usr/bin/env python3
"""Build the wholly original MIT VM86 guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/vm86.asm'
    boot = assemble(source)
    assert len(boot) == 8192
    image = boot + bytes(1_261_568-len(boot))
    result = bytearray([0xa5]*64)
    result[:6] = bytes([2, 4, 3, 2, 2, 1])
    for offset, value in [(8, 0xe0010011), (12, 0x55000), (16, 0x12345678),
                          (20, 0x23046), (24, 0xabcd7000), (28, 0x4000),
                          (32, 0x6000), (36, 0xa000), (40, 0x5000),
                          (44, 0x0800), (48, 0x9000), (52, 2), (56, 0)]:
        result[offset:offset+4] = value.to_bytes(4, 'little')
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/vm86.asm -o vm86.bin',
        'license_sha256': sha((ROOT/'LICENSE').read_bytes()),
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/vm86/vm86.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Original CPL0 IRETD -> CPL3 VM86 in PG=0/1 with nonidentity code/data/stack, complete context and guards, IOPL and bitmap #GP, cross-page opcodes/operands, recoverable user #PF(4/5/7), protected trap/IRQ gates with TSS SS0:ESP0, guest handlers and IRETD, PIC/PIT IRQ0, text MMIO, cold/reset',
        'result_address': '0x8000', 'result_length': 64,
        'result_hex': result.hex(),
        'markers': ['VM86 PASS', 'VM86 FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/vm86-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('VM86 fixture differs from checked manifest')
    dest = ROOT/'fixtures/vm86'
    dest.mkdir(exist_ok=True)
    (dest/'vm86.hdm').write_bytes(image)
    (dest/'vm86.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('VM86 fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
