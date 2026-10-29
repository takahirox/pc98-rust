#!/usr/bin/env python3
"""Build the wholly original MIT task-register initialization guest offline."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/task_register.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568-len(boot))
    result = bytearray([0xa5]*64)
    result[1:4] = bytes([0x28, 0, 0x16])
    result[5:7] = bytes([0x28, 0])
    result[9:13] = bytes([0x33, 0, 0x33, 0])
    result[15:18] = bytes([0x33, 0, 0x32])
    result[19:23] = bytes([0x30, 0, 0x30, 0])
    result[25:28] = bytes([0x30, 0, 0x46])
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/task_register.asm -o task_register.bin',
        'source_sha256': sha((ROOT/source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/task_register/task_register.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'contract': 'CPL0 PE=1 PG=0 LTR16/STR register and memory, 16/32-bit TSS, 66/67, flags, GDT busy bits, guest reload, TR survives PE exit/reentry, 64 result/guard bytes, cold/reset',
        'result_address': '0x8000', 'result_length': 64,
        'result_hex': result.hex(),
        'markers': ['TASK REGISTER PASS', 'TASK REGISTER FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    path = ROOT/'fixtures/task-register-manifest.json'
    if update:
        path.write_text(text)
    elif path.read_text() != text:
        raise ValueError('Task-register fixture differs from checked manifest')
    dest = ROOT/'fixtures/task_register'
    dest.mkdir(exist_ok=True)
    (dest/'task_register.hdm').write_bytes(image)
    (dest/'task_register.asm').write_bytes((ROOT/source).read_bytes())
    (dest/'LICENSE').write_bytes((ROOT/'LICENSE').read_bytes())
    (dest/'manifest.json').write_text(text)
    print('Task-register fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
