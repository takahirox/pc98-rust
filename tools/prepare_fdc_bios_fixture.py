#!/usr/bin/env python3
"""Build the original MIT BIOS/FDC handoff probe offline and check its hashes."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/fdc_bios.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = bytearray((i * 17 + i // 1024 * 23) & 255 for i in range(1_261_568))
    image[:1024] = boot
    image[1024:2048] = bytes((i ^ (i >> 8)) & 255 for i in range(1024))
    image[33792:34816] = bytes((7 * i + 0x31) & 255 for i in range(1024))
    expected_media = image.copy()
    expected_media[2048:3072] = bytes((29 * i + 71) & 255 for i in range(1024))
    expected_media[34816:35840] = bytes((13 * i + 0xb3) & 255 for i in range(1024))
    record = bytes([1] + [0] * 14 + [0xa5] * 17)
    firmware = assemble('guest/firmware.asm')
    manifest = {
        'source': source,
        'license': 'MIT; original guest, synthetic firmware source and LICENSE copied alongside image',
        'assembly': 'nasm -f bin guest/fdc_bios.asm -o fdc_bios.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/fdc_bios/fdc_bios.hdm',
        'image_sha256': sha(image),
        'firmware_source_sha256': sha((ROOT / 'guest/firmware.asm').read_bytes()),
        'firmware_sha256': sha(firmware),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap'],
        'provenance': 'Original source-built bare-metal MIT probe; no acquired binary, commercial ROM, DOS or copied emulator implementation',
        'contract': 'Fresh SPECIFY ND=1 independently before BIOS reads C0/C2 R2 and writes C0/C2 R3; BIOS readback of both writes; completed PIO C2/R2 -> BIOS C0/R2 -> fresh SPECIFY ND=1 -> PIO C0/R2. Actual INT 1Bh AL=90h AH=06h/05h CH=3 DH=0 BX=1024 ES:BP=buffer. Guest checks every CF/AH, payload, guard, PIO MSR/result and final SP.',
        'record_address': '0x8000',
        'record_hex': record.hex(),
        'record_layout': '8000=PASS done; 8001..800e=seven AH/CF pairs, all zero; rest A5',
        'read_patterns': {'C0H0R2': '(i ^ (i >> 8)) & 255', 'C2H0R2': '(7*i + 0x31) & 255'},
        'write_patterns': {'C0H0R3': '(29*i + 71) & 255', 'C2H0R3': '(13*i + 0xb3) & 255'},
        'buffers': {'0x9000': 'C0H0R2', '0x9800': 'C2H0R2', '0xa000': 'C0H0R3', '0xa800': 'C2H0R3', '0xb000': 'C2H0R3 readback', '0xb800': 'C2H0R2 PIO', '0xc000': 'C0H0R2 PIO'},
        'guards': 'All RAM 800f..c40f outside the seven 1024-byte buffers remains A5; host compares the complete 8000..c40f region',
        'pio_result': '40 80 00 C 00 02 03; EOT without TC, separate result phase after 1024 EXM data bytes',
        'expected_media_sha256': sha(expected_media),
        'media': 'Exactly two writes: only C0/H0/R3 (2048..3071) and C2/H0/R3 (34816..35839); hosts compare complete exported media to independently constructed expected bytes',
        'markers': ['FDC BIOS PASS', 'FDC BIOS FAIL'],
        'slot_budget': 2_000_000,
        'host': 'Mount, step, observe and reset only; no injected code, guest results or IRQs',
        'reset': 'Same image retained across reset; identical results and idempotent writes',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    checked = ROOT / 'fixtures/fdc-bios-manifest.json'
    if update:
        checked.write_text(text)
    elif checked.read_text() != text:
        raise ValueError('FDC BIOS fixture differs from fixtures/fdc-bios-manifest.json')
    destination = ROOT / 'fixtures/fdc_bios'
    destination.mkdir(exist_ok=True)
    (destination / 'fdc_bios.hdm').write_bytes(image)
    for path in [source, 'guest/firmware.asm', 'LICENSE']:
        (destination / path.split('/')[-1]).write_bytes((ROOT / path).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('FDC BIOS fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
