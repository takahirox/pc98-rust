#!/usr/bin/env python3
"""Build the original MIT OCW3 nesting probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pic_smm.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([
        0xa5, 2, 3, 1, 0x1c, 3, 0, 1,
        1, 0, 1, 0xa5, 2, 3, 2, 0xa5,
        1, 0xfe, 0, 0, 0xa5, 0xa5, 0xa5, 0xa5,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pic_smm.asm -o pic_smm.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pic_smm/pic_smm.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Hardware keyboard IR1 outer handler with IR0 lowest; masked ISR1 blocks confirmed pending PIT IR0 in normal mode with IF set, then OCW3 68h admits nesting; nested nonspecific EOI preserves ISR1; outer specific EOI clears it',
        'handshake': 'At guest phase byte 0x8001 = 1 send physical Return down using existing input. Guest confirms IRR & 3 == 3 before STI, observes 256 normal-mode iterations, then enables SMM. Host never injects IRQs or writes guest results.',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'trace_address': '0x8008',
        'trace_hex': '010001',
        'isr_address': '0x800c',
        'isr_hex': '020302',
        'timer_count_before_address': '0x8006',
        'timer_count_after_address': '0x8007',
        'timer_count_before_after_hex': '0001',
        'markers': ['PIC SMM PASS', 'PIC SMM FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, handshake, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pic-smm-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIC SMM fixture differs from fixtures/pic-smm-manifest.json')
    destination = ROOT / 'fixtures/pic_smm'
    destination.mkdir(exist_ok=True)
    (destination / 'pic_smm.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pic_smm.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIC SMM fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
