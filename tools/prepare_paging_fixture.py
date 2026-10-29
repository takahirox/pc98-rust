#!/usr/bin/env python3
"""Build the wholly original MIT paging guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/paging.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568-len(boot))
    result = bytearray([0xa5]*64)
    result[1] = 1
    for offset, value in [(4, 0xe0010011), (8, 0x10018), (12, 0x403000),
                          (16, 0x11223344), (20, 0xcafebabe), (24, 0x60010011)]:
        result[offset:offset+4] = value.to_bytes(4, 'little')
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/paging.asm -o paging.bin',
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/paging/paging.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Original ring-0 nonidentity code/data/stack, physical two-level tables, PG on/off, CR0/CR3/CR2/SMSW, guest #PF frame validation and IRETD retry, stale TLB/INVLPG/CR3 reload, 16/32-bit code and guards, cold/reset',
        'result_address': '0x8000', 'result_length': 64,
        'result_hex': result.hex(),
        'markers': ['PAGING PASS', 'PAGING FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/paging-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('PAGING fixture differs from checked manifest')
    dest = ROOT/'fixtures/paging'
    dest.mkdir(exist_ok=True)
    (dest/'paging.hdm').write_bytes(image)
    (dest/'paging.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('PAGING fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
