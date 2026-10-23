#!/usr/bin/env python3
"""Build the original MIT V30 segment interrupt probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/v30_segment_irq.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 8, 8, 1, 1, 1] + [0xa5] * 10 + [1] * 8 + [0xa5] * 24)
    assert len(result) == 48
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/v30_segment_irq.asm -o v30_segment_irq.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/v30_segment_irq/v30_segment_irq.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM'],
        'provenance': 'Original source; no DOS, proprietary firmware, commercial media or copied emulator implementation',
        'contract': 'Eight PIT/PIC-produced IRQs: segment-write/read register and odd memory MOVs, POP, consecutive producers, prefixed following INC; guest verifies progress before entry, exact return IP/CS, six-byte stack, saved IF, ISR/IRR, EOI/IRET, single entry per case, results and guards before PASS.',
        'specification': 'NEC V20/V30 User Manual, Chapter 6, printed page 6-4, Interrupt Disable Timing',
        'firmware_source': 'guest/firmware.asm (original MIT synthetic IPL)',
        'firmware_sha256': sha(assemble('guest/firmware.asm')),
        'host': 'Boot, reset, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8000=a5 guard; 8001=8 cases, 8002=8 entries, 8003=1 confirmed IRR, 8004=1 following progress, 8005=1 ISR in handler; 8006..800f=a5 guards; 8010..8017=1 entry per case; 8018..802f=a5 guards',
        'destination_address': '0x9000',
        'destination_hex': 'a5c01f5a',
        'guards': '9000=a5, 9003=5a surround odd segment word 9001=1fc0; all result and guard bytes checked by guest and hosts',
        'markers': ['V30 SEGMENT IRQ PASS', 'V30 SEGMENT IRQ FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Identical image, expected bytes and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/v30-segment-irq-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('V30 segment interrupt fixture differs from fixtures/v30-segment-irq-manifest.json')
    destination = ROOT / 'fixtures/v30_segment_irq'
    destination.mkdir(exist_ok=True)
    (destination / 'v30_segment_irq.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'v30_segment_irq.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('V30 SEGMENT IRQ fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
