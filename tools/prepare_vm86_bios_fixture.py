#!/usr/bin/env python3
"""Build the wholly original MIT VM86 BIOS guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/vm86_bios.asm'
    boot = assemble(source)
    assert len(boot) == 8192
    image = boot + bytes(1_261_568-len(boot))
    result = bytes([3, 0, 4, 0])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/vm86_bios.asm -o vm86_bios.bin',
        'license_sha256': sha((ROOT/'LICENSE').read_bytes()),
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/vm86_bios/vm86_bios.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'Original VM86 monitor reflects timer/keyboard IRQ and BIOS INT gates; PG=0, identity and nonidentity paging; work-area and cross-page buffer #PF with repair/retry; flags, callbacks, blocking keyboard; cold/reset',
        'result_address': '0x8000', 'result_length': 4,
        'result_hex': result.hex(),
        'markers': ['VM86 BIOS PASS', 'VM86 BIOS FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/vm86-bios-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('VM86 BIOS fixture differs from checked manifest')
    dest = ROOT/'fixtures/vm86_bios'
    dest.mkdir(exist_ok=True)
    (dest/'vm86_bios.hdm').write_bytes(image)
    (dest/'vm86_bios.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('VM86 BIOS fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
