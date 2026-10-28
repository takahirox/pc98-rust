#!/usr/bin/env python3
"""Build the original MIT V30 ADD4S packed-BCD addition probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/v30_add4s.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    # Literal guest results, independently pinned (not computed by BCD code).
    result = bytes.fromhex(
        'a5' '100000000900' '000041040100' '001000040100'
        '000041000180' '010000040000' 'a5')
    assert len(result) == 32
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/v30_add4s.asm -o v30_add4s.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/v30_add4s/v30_add4s.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM'],
        'provenance': 'Original source; no DOS, proprietary firmware, commercial media or copied emulator implementation',
        'contract': 'Five literal ADD4S cases: 09+01, 9999+0001, 0999+0001 at CL=3, 1999+8001 at CL=3, 0001+0000 with last result byte zero. Incoming CF ignored; DF varied; SI/DI/CX and AX/BX/DX/DS/ES/SP checked. Guest checks source, destination, guards, all result bytes and flags masked by 0x0641 (CF/ZF/IF/DF) before PASS; undefined status flags excluded.',
        'host': 'Boot, reset, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8000 guard; five 6-byte records at 8001,8007,800d,8013,8019: destination word, masked FLAGS word, unchanged source word; 801f guard',
        'destination_address': '0x9000',
        'destination_hex': 'a500005a000000000000000000000000a501005a',
        'guards': 'Source 9000=a5,9003=5a; destination 9010=a5,9013=5a checked after each case. CL=2 also checks unused high operand bytes unchanged. Results 8000 and 801f=a5; exact 32-byte result checked by guest and hosts. Hosts check complete final 9000..9013 span.',
        'markers': ['V30 ADD4S PASS', 'V30 ADD4S FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Identical image, expected bytes and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/v30-add4s-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('V30 ADD4S packed-BCD addition fixture differs from fixtures/v30-add4s-manifest.json')
    destination = ROOT / 'fixtures/v30_add4s'
    destination.mkdir(exist_ok=True)
    (destination / 'v30_add4s.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'v30_add4s.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('V30 ADD4S fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
