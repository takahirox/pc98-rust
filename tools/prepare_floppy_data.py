#!/usr/bin/env python3
"""Build independent MIT FAT12 data media; no acquired boot code or DOS files."""
import argparse
import hashlib
import json
import struct
from build_media import ROOT
from prepare_freedos import image_with_files


def d88(raw):
    image = bytearray(0x2b0)
    image[:12] = b'PC98 DATA126'
    image[0x1b] = 0x20
    for track in range(154):
        struct.pack_into('<I', image, 0x20 + track * 4, len(image))
        for sector in range(8):
            header = bytearray(16)
            header[:6] = bytes([track // 2, track % 2, sector + 1, 3, 8, 0])
            struct.pack_into('<H', header, 14, 1024)
            image += header
            offset = (track * 8 + sector) * 1024
            image += raw[offset:offset + 1024]
    struct.pack_into('<I', image, 0x1c, len(image))
    return bytes(image)


def build(update=False):
    outputs = {}
    for serial, name, contents in [
        (12601, b'SECOND  TXT', b'ORIGINAL FDD2 FILE CONTENTS\r\n'),
        (12602, b'SWAPPED TXT', b'REPLACEMENT FDD2 FILE CONTENTS\r\n'),
    ]:
        boot = bytearray(1024)
        boot[:11] = b'\xeb\xfe\x90PC98DATA'
        struct.pack_into('<HBHBHHBHHHII', boot, 11,
                         1024, 1, 1, 2, 192, 1232, 0xfe, 2, 8, 2, 0, 0)
        boot[38] = 0x29
        struct.pack_into('<I', boot, 39, serial)
        boot[43:54] = b'DATA126    '
        boot[54:62] = b'FAT12   '
        boot[510:512] = b'\x55\xaa'
        raw = bytearray(image_with_files(boot, {name: contents}))
        struct.pack_into('<H', raw, 5 * 1024 + 24, 0x5c21) # 2026-01-01
        raw = bytes(raw)
        stem = 'second' if serial == 12601 else 'swapped'
        outputs[stem + '.hdm'] = raw
        outputs[stem + '.d88'] = d88(raw)
    manifest = {
        'license': 'MIT; original data/BPB/container, no third-party guest code',
        'sources': {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                    for name in ['tools/prepare_floppy_data.py', 'tools/prepare_freedos.py',
                                 'tools/build_media.py', 'LICENSE']},
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes; FAT12',
        'files': {'SECOND.TXT': 'ORIGINAL FDD2 FILE CONTENTS',
                  'SWAPPED.TXT': 'REPLACEMENT FDD2 FILE CONTENTS'},
        'images': {name: hashlib.sha256(data).hexdigest() for name, data in outputs.items()},
    }
    text = json.dumps(manifest, indent=2) + '\n'
    checked = ROOT / 'fixtures/floppy-data-manifest.json'
    if update:
        checked.write_text(text)
    elif checked.read_text() != text:
        raise ValueError('floppy data manifest differs')
    dest = ROOT / 'fixtures/floppy_data'
    dest.mkdir(exist_ok=True)
    for name, data in outputs.items():
        (dest / name).write_bytes(data)
    for name in ['tools/prepare_floppy_data.py', 'tools/prepare_freedos.py', 'tools/build_media.py', 'LICENSE']:
        (dest / name.split('/')[-1]).write_bytes((ROOT / name).read_bytes())
    (dest / 'manifest.json').write_text(text)
    print('Independent FAT12/D88 data verified')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
