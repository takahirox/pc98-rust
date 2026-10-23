#!/usr/bin/env python3
"""Build the original MIT V30 BOUND next-IP probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/v30_bound.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes.fromhex('a502020501c01fd7f54501c01fd7f75aa5fbff0a00' + 'a5' * 11)
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/v30_bound.asm -o v30_bound.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/v30_bound/v30_bound.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM'],
        'provenance': 'Original source; no DOS, commercial ROM, guest binary or copied emulator implementation',
        'contract': 'Signed inclusive checks at -5, 0 and 10 preserve index and FLAGS. Below (-6) and above (11) invoke vector 5 with IF clear/set. Guest checks entry IF/TF, SP, exact next IP including ES prefix, nonzero CS, original FLAGS, unmodified IRET, two entries and two forward-progress results; compares all 32 result/bounds/guard bytes before PASS',
        'host': 'Boot, step and read only; no input, IRQ injection or result writes',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': 'All a5 bytes in result_hex must remain unchanged; guest and host compare all 32 bytes',
        'result_layout': '8000 guard; 8001 entries=2; 8002 progress=2; 8003..8008 below frame; 8009..800e above frame; 800f status=5a; 8010 guard; 8011..8014 bounds=-5,10; 8015..801f guards',
        'markers': ['V30 BOUND PASS', 'V30 BOUND FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, results and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/v30-bound-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('V30 BOUND fixture differs from fixtures/v30-bound-manifest.json')
    destination = ROOT / 'fixtures/v30_bound'
    destination.mkdir(exist_ok=True)
    (destination / 'v30_bound.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'v30_bound.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('V30 BOUND fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
