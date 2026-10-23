#!/usr/bin/env python3
"""Build the original MIT V30 memory nibble-rotation probe offline, outside web/."""
import argparse
import hashlib
import json
import struct
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/v30_nibble_mem.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    records = [(0x23, 1, 0x5a, 0xfcd7), (0x31, 2, 0x6b, 0xf002),
               (0xfe, 0x0a, 0x7c, 0xf002), (0xea, 0x0f, 0x8d, 0xfcd7)]
    result = b'\xa5' + b''.join(struct.pack('<4B2H', d, al, ah, 0xa5, flags, flags)
                                for d, al, ah, flags in records) + b'\xa5' * 3
    assert len(result) == 36
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/v30_nibble_mem.asm -o v30_nibble_mem.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/v30_nibble_mem/v30_nibble_mem.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM'],
        'provenance': 'Original source; no DOS, proprietary firmware, commercial media or copied emulator implementation',
        'contract': 'Memory /0 ROL4 and ROR4; decimal and nondecimal nibbles; guest checks masked AL, AH, complete before/after FLAGS, every result/guard byte and SP before PASS. AL upper nibble is unspecified by hardware and is not asserted by this guest.',
        'host': 'Boot, reset, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8000 guard; four 8-byte records at 8001,8009,8011,8019: destination byte, AL&0f, AH, guard, FLAGS after, FLAGS before (little-endian words); 8021..8023 guards',
        'destination_address': '0x9000',
        'destination_hex': 'a5ea5a',
        'guards': 'Immediate operand neighbors 9000=a5 and 9002=5a checked after every rotation; all eight a5 bytes and all record bytes in the exact 36-byte result are checked by guest and hosts',
        'markers': ['V30 NIBBLE MEM PASS', 'V30 NIBBLE MEM FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Identical image, expected bytes and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/v30-nibble-mem-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('V30 memory nibble-rotation fixture differs from fixtures/v30-nibble-mem-manifest.json')
    destination = ROOT / 'fixtures/v30_nibble_mem'
    destination.mkdir(exist_ok=True)
    (destination / 'v30_nibble_mem.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'v30_nibble_mem.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('V30 NIBBLE MEM fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
