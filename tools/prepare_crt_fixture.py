#!/usr/bin/env python3
"""Build the original MIT CRT family guest and independent old-failure entries."""
import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/crt.asm'
    boot = assemble(source)
    assert len(boot) == 4096
    image = boot + bytes(1_261_568 - len(boot))
    result = bytearray([0xa5] * 128)
    result[1] = 0x5a
    rom = (ROOT / 'fixtures/fonts/jis0208.bin').read_bytes()
    result[4] = rom[((0x24 - 0x21) * 94 + 0x22 - 0x21) * 32]
    result[10:18] = bytes([0x38, 0x44, 0x44, 0x7c, 0x44, 0x44, 0x44, 0])
    pattern = bytes(0x80 >> (i // 2 % 8) if i % 2 == 0 else (i * 7) ^ 0x53 for i in range(32))
    result[34:66] = pattern
    result[68:100] = pattern
    manifest = {
        'source': source,
        'license': 'MIT; original pc98-rust contributors; LICENSE',
        'provenance': 'Original x86 probe, no ROM, private game, third-party font or DOS',
        'assembly': 'nasm -f bin guest/crt.asm -o crt.bin; -DFIRST=0xNN for independent entry',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024-byte sectors',
        'profile': 'Ap, including explicit normal-resolution compatibility extension for 1Ch..1Eh',
        'contract': 'INT/ROM/IRET register and flags preservation; KCG ROM/user RAM BIOS/port/window agreement; mode set/sense; four staged real scanouts: split/pitch/cursor, dot restriction, blink, 40x20; restore',
        'result_address': '0x8000',
        'result_length': 128,
        'result_hex': result.hex(),
        'stages': [1, 2, 3, 4, 90],
        'slot_budget_per_stage': 200_000,
        'reset': 'identical disk, guest sequence and assertions after CPU reset',
        'red_before': ['0fh', '1bh', '1ch', '1dh', '1eh'],
        'distribution': 'Generated fixtures/crt only; excluded from public web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/crt-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('CRT fixture differs from fixtures/crt-manifest.json')
    dest = ROOT / 'fixtures/crt'
    dest.mkdir(exist_ok=True)
    (dest / 'crt.hdm').write_bytes(image)
    (dest / 'crt.asm').write_bytes((ROOT / source).read_bytes())
    (dest / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (dest / 'manifest.json').write_text(text)
    for ah in [0x0f, 0x1b, 0x1c, 0x1d, 0x1e]:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / 'entry.bin'
            subprocess.run(['nasm', '-f', 'bin', f'-DFIRST={ah}', str(ROOT / source), '-o', str(binary)], check=True)
            entry = binary.read_bytes()
        (dest / f'first-{ah:02x}.hdm').write_bytes(entry + bytes(len(image) - len(entry)))
    print('CRT fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
