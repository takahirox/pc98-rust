#!/usr/bin/env python3
"""Build the original MIT OCW3 polling probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pic_poll.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([
        0xa5, 0x80, 1, 0, 0x87, 0, 0x80, 0x83, 8, 0x80, 0, 0, 0, 0, 1, 1,
        0, 0, 0, 0, 0, 0, 1, 3, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pic_poll.asm -o pic_poll.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pic_poll/pic_poll.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'CLI; guest-programmed one-shot PIT IRQ0 poll, master IR7 and separate slave FDC READ ID IR3 poll, ISR snapshots and separate normal EOIs, empty polls, then STI and ordinary PIT IRQ0 delivery',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'results': 'Offsets 1-3: PIT poll/ISR/EOI; 4-8: master poll/slave ISR/master ISR/slave poll/slave ISR; 9-11: separate EOI ISR snapshots; 12-13: empty master/slave polls; 14-16: ordinary PIT count/ISR/post-EOI ISR; 17-23: FDC READ ID result',
        'markers': ['PIC POLL PASS', 'PIC POLL FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pic-poll-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIC POLL fixture differs from fixtures/pic-poll-manifest.json')
    destination = ROOT / 'fixtures/pic_poll'
    destination.mkdir(exist_ok=True)
    (destination / 'pic_poll.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pic_poll.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIC POLL fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
