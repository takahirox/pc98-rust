#!/usr/bin/env python3
"""Build the original MIT Ap FPU boot probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/fpu_init.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes.fromhex(
        'a50000cdaba5a5a5 a57f03a5a5a5a5a5'
        'a50000a5a5a5a5a5 a57f03a5a5a5a5a5'
        'a50000a5a5a5a5a5 a57f030000a5a5a5'
        'a50000efcda5a5a5 a50204a5a5a5a5a5')
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/fpu_init.asm -o fpu_init.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/fpu_init/fpu_init.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profile': 'Ap real mode; no DOS, proprietary ROM or emulator code',
        'contract': 'FNINIT, FNSTCW m16, FNSTSW m16/AX, integer flags and EAX high half, repeated size and last segment overrides, FS/GS, SS SIB, exact stores, FFFE, atomic DS/SS #GP, LOCK #UD on all four forms, six-byte first-prefix frames, guest repair/IRET/retry, cold and reset boots',
        'result_address': '0x8000',
        'result_length': 64,
        'result_hex': result.hex(),
        'last_operand_address': '0xfffd',
        'last_operand_hex': 'a57f03',
        'markers': ['FPU PASS', 'FPU FAIL'],
        'slot_budget': 2_000_000,
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/fpu_init-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('FPU fixture differs from fixtures/fpu_init-manifest.json')
    destination = ROOT / 'fixtures/fpu_init'
    destination.mkdir(exist_ok=True)
    (destination / 'fpu_init.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'fpu_init.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('FPU fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
