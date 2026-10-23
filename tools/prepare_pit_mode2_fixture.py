#!/usr/bin/env python3
"""Build the original MIT binary mode-2 probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pit_mode2.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 1, 2, 1, 1, 0, 0, 1] + [0xa5] * 24)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pit_mode2.asm -o pit_mode2.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pit_mode2/pit_mode2.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Latch live count from divisor 60000, rewrite 2000, confirm live count remains above 55000 and advances by less than 100; reinitialize PIC to discard setup edges; service exactly two periodic IRQ0 entries with ISR readback, EOI and IRET, confirming count <=2000 in the new periods',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'result_layout': '8000 guard; 8001 deferred-reload proof=1; 8002 periodic entries=2; 8003/8004 ISR entry=1/1; 8005/8006 post-EOI ISR=0/0; 8007 new-period count proof=1; 8008..801f guards=a5',
        'markers': ['PIT MODE2 PASS', 'PIT MODE2 FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pit-mode2-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIT MODE2 fixture differs from fixtures/pit-mode2-manifest.json')
    destination = ROOT / 'fixtures/pit_mode2'
    destination.mkdir(exist_ok=True)
    (destination / 'pit_mode2.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pit_mode2.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIT MODE2 fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
