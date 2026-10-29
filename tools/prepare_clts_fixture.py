#!/usr/bin/env python3
"""Build the wholly original MIT CLTS initialization guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/clts.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568-len(boot))
    result = bytearray([0xa5]*64)
    for offset in [0, 16]:
        result[offset+1:offset+8] = bytes([6, 1, 0xa5, 0x7f, 3, 0, 0])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/clts.asm -o clts.bin',
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/clts/clts.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Real and CPL0 PE=1 PG=0: LMSW/MOV CR0 TS -> four supported x87 #NM forms and WAIT MP/TS, frame validation, CLTS/IRET retry, EM survives CLTS; exact results/guards, cold/reset',
        'result_address': '0x8000', 'result_length': 64,
        'result_hex': result.hex(),
        'markers': ['CLTS PASS', 'CLTS FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/clts-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('CLTS fixture differs from checked manifest')
    dest = ROOT/'fixtures/clts'
    dest.mkdir(exist_ok=True)
    (dest/'clts.hdm').write_bytes(image)
    (dest/'clts.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('CLTS fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
