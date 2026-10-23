#!/usr/bin/env python3
"""Build the original MIT V30 carry-repeat probe offline, outside web/."""
import argparse
import hashlib
import json
import struct
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/v30_repc.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    records = [
        (1, 0x303, 0x9003, 0), (0, 0x308, 0x9008, 0x95),
        (1, 0x307, 0x9007, 0), (0, 0x317, 0x9017, 0x95),
        (1, 0x301, 0x9005, 0), (1, 0x305, 0x9008, 0x95),
        (1, 0x30b, 0x9011, 0), (0, 0x311, 0x9017, 0x95),
        (0, 0xffff, 0xffff, 0x8d5), (0, 0x302, 0x9002, 0x95),
    ]
    result = b'\xa5' + b''.join(struct.pack('<4H', *r) for r in records) + b'\x5a' + b'\xa5' * 14
    src = bytes.fromhex('a5010500a5050505a5050001000000a5a5050005000500' + 'a5' * 9)
    dst = bytes.fromhex('a5020400a5050406a5040002000000a5a5050004000600' + 'a5' * 9)
    assert len(result) == 96 and len(src) == len(dst) == 32
    assert boot[0x300:0x320] == src
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/v30_repc.asm -o v30_repc.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/v30_repc/v30_repc.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM'],
        'provenance': 'Original source; no DOS, proprietary firmware, commercial media or copied emulator implementation',
        'contract': 'Both carry prefixes and CMPSB/W, SCASB/W; opposite incoming CF, equality continuing REPNC, early stops, exhaustion, backward unaligned words, distinct CS/ES and both segment-prefix orders, zero/one counts. Guest checks all result, operand and guard bytes and SP before PASS.',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8000 guard; 8001..8050 ten records of CX,SI,DI,FLAGS&08d5h; 8051 status=5a; 8052..805f guards',
        'source_address': '0x1ff00',
        'source_hex': src.hex(),
        'destination_address': '0x9000',
        'destination_hex': dst.hex(),
        'guards': 'All a5 bytes in result/source/destination hex remain unchanged; guest and host check all 160 bytes',
        'markers': ['V30 REPC PASS', 'V30 REPC FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Identical image, expected bytes and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/v30-repc-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('V30 carry-repeat fixture differs from fixtures/v30-repc-manifest.json')
    destination = ROOT / 'fixtures/v30_repc'
    destination.mkdir(exist_ok=True)
    (destination / 'v30_repc.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'v30_repc.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('V30 REPC fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
