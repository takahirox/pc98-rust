#!/usr/bin/env python3
"""Build the original MIT OCW2 boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pic.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([
        0xa5, 4, 6, 0xa5, 0x1c, 0x9c, 0x1c, 0xa5,
        1, 0, 0, 1, 1, 0, 0xa5, 0xa5,
        3, 3, 3, 0xa5, 2, 1, 1, 2, 2, 1,
        0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5,
    ])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pic.asm -o pic.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pic/pic.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Hardware PIT IR0 and keyboard IR1; set IR0 lowest; rotating nonspecific EOI makes IR1 lowest; rotating specific EOI makes IR0 lowest; ordinary EOIs preserve priority; exact trace 1,0,0,1,1,0',
        'handshake': 'At guest phase byte 0x8001 = 1,2,3 inject Return down,up,down using existing input. Guest waits for IRR & 3 == 3 before each STI and stores 03 at 0x8010..0x8012. Host never writes guest memory or results.',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'trace_address': '0x8008',
        'trace_hex': '010000010100',
        'isr_address': '0x8014',
        'isr_hex': '020101020201',
        'markers': ['PIC PASS', 'PIC FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, handshake, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pic-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIC fixture differs from fixtures/pic-manifest.json')
    destination = ROOT / 'fixtures/pic'
    destination.mkdir(exist_ok=True)
    (destination / 'pic.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pic.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIC fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
