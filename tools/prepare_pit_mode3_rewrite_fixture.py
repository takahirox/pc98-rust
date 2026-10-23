#!/usr/bin/env python3
"""Build the original MIT binary mode-3 rewrite probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pit_mode3_rewrite.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 1, 2, 1, 1, 0, 0, 1, 1] + [0xa5] * 23)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pit_mode3_rewrite.asm -o pit_mode3_rewrite.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pit_mode3_rewrite/pit_mode3_rewrite.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Program paired binary mode 3 once, count 60000; two latched even counts >=55000 separated by 500 LOOPs prove decrement by two (delta 1900..2200); complete paired replacement 2000 without mode word; after 500 LOOPs latch old count 52000..56000 (immediate restart would be <=2000); initialize PIC discarding setup rise, unmask IRQ0, bounded 20000-iteration wait for exactly two natural IRQ0 entries; each checks ISR=1, latched even count 2..2000, EOI, ISR=0 and IRET; mask after second entry; no count reads between paired write bytes',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'result_layout': '8000 guard; 8001 old-count decrement-by-two=1; 8002 IRQ0 entries=2; 8003/8004 ISR entry=1/1; 8005/8006 after EOI=0/0; 8007 new divisor count proof=1; 8008 deferred old half proof=1; 8009..801f guards=a5',
        'markers': ['PIT MODE3 REWRITE PASS', 'PIT MODE3 REWRITE FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pit-mode3-rewrite-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIT MODE3 REWRITE fixture differs from fixtures/pit-mode3-rewrite-manifest.json')
    destination = ROOT / 'fixtures/pit_mode3_rewrite'
    destination.mkdir(exist_ok=True)
    (destination / 'pit_mode3_rewrite.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pit_mode3_rewrite.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIT MODE3 REWRITE fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
