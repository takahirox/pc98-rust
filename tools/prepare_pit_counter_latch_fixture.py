#!/usr/bin/env python3
"""Build the original MIT ordinary counter-latch retention probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/pit_counter_latch.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 0x0d, 0x12, 0x54, 0x10, 0x0b, 0x12, 0x56, 0x10] + [0xa5] * 23)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/pit_counter_latch.asm -o pit_counter_latch.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/pit_counter_latch/pit_counter_latch.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap real mode'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Program paired binary mode 0 count 1234h separately for each case; after 16 LOOPs latch, delay 200 LOOPs, issue multiple duplicates before any read or between bytes; use both port banks for commands and reads; require retained 12xxh snapshot, then recapture a current 0e00h..10ffh count at least 100h lower; guest compares all raw bytes and guards before PASS',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'result_layout': '8000 guard; 8001/8002 retained low/high before any read; 8003/8004 recapture low/high; 8005/8006 retained low/high with duplicate between bytes; 8007/8008 recapture low/high; 8009..801f guards=a5',
        'markers': ['PIT COUNTER LATCH PASS', 'PIT COUNTER LATCH FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/pit-counter-latch-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('PIT COUNTER LATCH fixture differs from fixtures/pit-counter-latch-manifest.json')
    destination = ROOT / 'fixtures/pit_counter_latch'
    destination.mkdir(exist_ok=True)
    (destination / 'pit_counter_latch.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'pit_counter_latch.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('PIT COUNTER LATCH fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
