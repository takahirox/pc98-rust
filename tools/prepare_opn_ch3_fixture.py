#!/usr/bin/env python3
"""Build the original MIT channel-3 PCM probe offline, outside web/."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build(update=False):
    source = 'guest/opn_ch3.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result = bytes([0xa5, 0xff] + [0xa5] * 14 + [1] * 16 + [0xa5] * 32)
    pairs = [(0x123, 3), (0x456, 4), (0x789, 2), (0x5ab, 5)]
    stages = []
    for operator in range(1, 5):
        for mode in [0, 0x40, 0x80, 0]:
            fnum, block = pairs[operator - 1 if mode else 3]
            stages.append({'stage': len(stages) + 1, 'operator': operator,
                           'mode': mode, 'fnum': fnum, 'block': block})
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE and original assembly copied alongside image',
        'assembly': 'nasm -f bin guest/opn_ch3.asm -o opn_ch3.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/opn_ch3/opn_ch3.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['Ap real mode; shared OPN/OPNA core; VM sound remains absent'],
        'provenance': 'Original MIT assembly; no acquired guest binaries, proprietary ROMs, samples or emulator code',
        'contract': 'Algorithm 7, single keyed logical operator, DT=0 MULT=1 AR=31 DR=SR=0; high before low; each operator normal/40h/80h/normal without key edges at mode changes; Timer A/B stopped',
        'host': 'Load, step, reset, read memory/text/disk and drain existing PCM queue only; no register or result injection',
        'stage_address': '0x8001',
        'stages': stages,
        'stage_duration': '80000 LOOP instructions, about 3840 samples at 48 kHz',
        'pcm_assertions': 'At least 3500 samples/stage; finite and bounded; interpolated rising crossings >=3; pitch within max(0.03 Hz, 0.02 percent); chunk 1000 slots / 48 samples prevents 4096-sample queue overflow; discard chunks crossing markers',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'guards': '8000h,8002h..800Fh,8020h..803Fh stay A5; 8001h=FF done; 8010h..801Fh=1 stage completions; guest and hosts compare all 64 bytes',
        'markers': ['OPN CH3 PASS', 'OPN CH3 FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same image, PCM assertions, results and budget before and after Machine reset',
        'unverified': 'Combined mode 11, CSM automatic timer keying, write latch timing, prescalers, key scaling, fuller envelopes/SSG-EG, LFO/stereo/rhythm/ADPCM; neither named special mode is fully implemented',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/opn-ch3-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('OPN CH3 fixture differs from fixtures/opn-ch3-manifest.json')
    destination = ROOT / 'fixtures/opn_ch3'
    destination.mkdir(exist_ok=True)
    (destination / 'opn_ch3.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'opn_ch3.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('OPN CH3 fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
