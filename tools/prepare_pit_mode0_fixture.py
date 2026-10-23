#!/usr/bin/env python3
"""Build the original MIT binary mode-0 probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pit_mode0.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 2, 1, 1, 0, 0, 1, 1, 1, 1] + [0xa5] * 22)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pit_mode0.asm -o pit_mode0.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pit_mode0/pit_mode0.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Initialize PIC after lowering OUT to discard firmware/setup requests; arm binary mode 0 divisor 2000 with a fresh mode word twice; bounded 6000-iteration IRQ wait; exactly one hardware IRQ0 per arm with ISR=1, EOI, ISR=0 and IRET; two latched post-terminal counts >=60000, first <65535, separated by 500 LOOP instructions, delta 900..1100; remain unmasked for 40000 LOOP instructions (>65536 PIT clocks) per arm, reject additional entries and require IRR0=ISR0=0',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'result_layout': '8000 guard; 8001 total IRQ0 entries=2; 8002/8003 ISR entry=1/1; 8004/8005 post-EOI=0/0; 8006/8007 post-terminal advancing counts=1/1; 8008/8009 no extra IRQ across continued wrap=1/1; 800a..801f guards=a5',
        'markers': ['PIT MODE0 PASS', 'PIT MODE0 FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pit-mode0-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIT MODE0 fixture differs from fixtures/pit-mode0-manifest.json')
    destination = ROOT / 'fixtures/pit_mode0'
    destination.mkdir(exist_ok=True)
    (destination / 'pit_mode0.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pit_mode0.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIT MODE0 fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
