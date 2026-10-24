#!/usr/bin/env python3
"""Build the original MIT SNGL interrupt probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pic_single.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    # Independently specified from the ICW/IRR/ISR contract, never emulator output.
    result = bytes([
        0xa5, 1, 1, 0, 0xfe, 0, 1, 0, 0, 0xfe, 0,
        1, 0x80, 0, 0, 8, 0x7f, 0xf7, 0,
        1, 0, 8, 0x83, 8, 0, 0,
        0, 0, 0, 0, 0, 1, 3,
        1, 0x80, 8, 0, 0, 0x80, 0, 0,
        0, 0, 0, 0, 0, 1, 3,
    ] + [0xa5] * 16)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pic_single.asm -o pic_single.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pic_single/pic_single.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Guest I/O and real interrupt entry/IRET: single-mode PIT normal EOI and AEOI, local master vector 27h from FDC READ ID slave INT, held INT without repeated entries, separate slave poll/drain, restored cascade vector 53h and separate normal EOIs',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 64 bytes',
        'results': '0 guard; 1-5 normal PIT count/ISR/IRR/IMR/post-EOI ISR; 6-10 AEOI PIT equivalents; 11-18 local IR7 count/master ISR/master IRR/slave ISR/slave IRR/master IMR/slave IMR/post-master-EOI ISR; 19-21 held INT count/master IRR/slave IRR; 22-25 slave poll/slave ISR/master ISR/post-slave-EOI ISR; 26-32 READ ID; 33-40 cascade count/master ISR/slave ISR/master IRR/slave IRR/master ISR after slave EOI/slave ISR after EOI/master ISR after EOI; 41-47 READ ID; 48-63 guards',
        'markers': ['PIC SINGLE PASS', 'PIC SINGLE FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pic-single-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIC SINGLE fixture differs from fixtures/pic-single-manifest.json')
    destination = ROOT / 'fixtures/pic_single'
    destination.mkdir(exist_ok=True)
    (destination / 'pic_single.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pic_single.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIC SINGLE fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
