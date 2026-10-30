#!/usr/bin/env python3
"""Build the original MIT Ap VM86 firmware boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/vm86_firmware.asm'
    boot = assemble(source)
    assert len(boot) == 8192
    image = boot + bytes(1_261_568 - len(boot))
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/vm86_firmware.asm -o vm86_firmware.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/vm86_firmware/vm86_firmware.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap paged VM86; no DOS, proprietary ROM or emulator code',
        'contract': 'Paged VM86 timer, callback, RTC across nonidentity pages, saved carry, 386 extended-copy descriptors, keyboard FIFO and recoverable user page faults; cold/reset',
        'markers': ['VM86 BIOS PASS', 'VM86 BIOS FAIL'],
        'slot_budget': 2_000_000,
        'result_address': '0x8000',
        'result_length': 8,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/vm86-firmware-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('VM86 firmware fixture differs from fixtures/vm86-firmware-manifest.json')
    destination = ROOT / 'fixtures/vm86_firmware'
    destination.mkdir(exist_ok=True)
    (destination / 'vm86_firmware.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'vm86_firmware.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('VM86 firmware fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
