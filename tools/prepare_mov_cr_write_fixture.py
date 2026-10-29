#!/usr/bin/env python3
"""Build the original MIT Ap MOV CR WRITE boot probe offline, outside web/."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/mov_cr_write.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytearray([0xa5] * 128)
    for i in range(8):
        value = 0x12345018 if i == 4 else 0x12340000 + i
        result[1+8*i:5+8*i] = value.to_bytes(4, 'little')
    result[65:69] = bytes.fromhex('32000560')
    result[73:77] = bytes.fromhex('18503412')
    result[81:83] = bytes.fromhex('3200')
    result[89:91] = bytes.fromhex('d708')
    result[97] = 4
    variants = {}
    variant_images = {}
    for name, value in [('pe', 0x60000011), ('pe_pg', 0xe0000011)]:
        with tempfile.TemporaryDirectory() as td:
            out = Path(td) / 'boot.bin'
            subprocess.run(['nasm', '-f', 'bin', f'-DMODE=0x{value:08x}',
                            str(ROOT / source), '-o', str(out)], check=True)
            variant_boot = out.read_bytes()
        assert len(variant_boot) == 1024
        variant_image = variant_boot + bytes(1_261_568 - 1024)
        variant_images[name] = variant_image
        variants[name] = {
            'assembly_define': f'MODE=0x{value:08x}',
            'boot_sha256': sha(variant_boot),
            'image_sha256': sha(variant_image),
            'written': f'{value:08x}',
            'expected': 'commit PE and halt with cached real-mode segments' if name == 'pe' else 'commit paging, then bounded triple-fault shutdown: no page tables installed',
            'cs_ip': '1fc0:00000007',
            'next_ip': '0000000d',
            'opcode_modrm': '66 67 65 0f 22 c0',
        }
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/mov_cr_write.asm -o mov_cr_write.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/mov_cr_write/mov_cr_write.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'Full-width CR2 writes from all GPRs, CR3 from ESP, CR0 60050032h and SMSW, preserved source/FLAGS, prefixes, guest #UD/#GP repair/IRET/retry, exact guards, cold boot and reset',
        'result_address': '0x8000',
        'result_length': 128,
        'result_hex': result.hex(),
        'markers': ['MOV CR WRITE PASS', 'MOV CR WRITE FAIL'],
        'slot_budget': 2_000_000,
        'transition_variants': variants,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/mov-cr-write-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('MOV CR WRITE fixture differs from fixtures/mov-cr-write-manifest.json')
    destination = ROOT / 'fixtures/mov_cr_write'
    destination.mkdir(exist_ok=True)
    (destination / 'mov_cr_write.hdm').write_bytes(image)
    for name, data in variant_images.items():
        (destination / f'{name}.hdm').write_bytes(data)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'mov_cr_write.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('MOV CR WRITE fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
