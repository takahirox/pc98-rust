#!/usr/bin/env python3
"""Build and verify the original MIT Time Set probe offline; no downloaded code."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


# Literal independent BIOS/serial byte oracle, not derived from production code
# or parsed from the guest's own comparison table.
RESULT = bytes.fromhex('''
    a5
    26 16 31 23 59 59
    26 16 31 23 59 59
    00 00 00 01 20
    26 20 01 00 00 00
    28 21 28 23 59 59
    28 21 28 23 59 59
    00 00 00 29 22 28 01
    28 22 29 00 00 00
    99 c5 31 23 59 59
    00 00 00 01 16 00 01
    00 16 01 00 00 00
    5a
''') + b'\xa5' * 59


def build(update=False):
    source = 'guest/rtc_set.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    assert len(RESULT) == 128
    image = boot + bytes(1_261_568 - len(boot))
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE; original pc98-rust contributors',
        'provenance': 'Original source-built probe; no commercial ROM, DOS or guest software',
        'assembler': 'nasm -f bin guest/rtc_set.asm -o rtc_set.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['Ap'],
        'initialization': 'Guest serially sets year 26, then loads parallel January 31, serial leap day and serial year rollover on each boot',
        'host': 'Mount identical image, step, inspect and reset only; no RTC injection or result patches',
        'contract': 'Parallel 010 and serial 0010 with full 52-bit chain; two >1-second held delays with CLK pulses; release 0/1/3; literal serial and BIOS checks; unrelated port bits and guards',
        'result_address': '0x8000',
        'result_length': 128,
        'result_hex': RESULT.hex(),
        'reset_result_hex': RESULT.hex(),
        'result_layout': '8000 guard; 8001..8006 parallel BIOS; 8007..800c held BIOS; 800d..8011 next-month serial; 8012..8017 BIOS; 8018..801d extended BIOS; 801e..8023 held BIOS; 8024..802a leap-day serial; 802b..8030 BIOS; 8031..8036 year-99 BIOS; 8037..803d year-00 serial; 803e..8043 BIOS; 8044 status=5a; 8045..807f guards=a5',
        'guards': '8000 and 8045..807f remain a5; guest and hosts compare every result/guard byte',
        'markers': ['RTC SET PASS', 'RTC SET FAIL'],
        'slot_budget': 8_000_000,
        'reset': 'Entire RTC retained; guest reprograms through ports for identical results on both boots',
        'distribution': 'Test only; generated fixtures/rtc_set excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/rtc-set-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('RTC Time Set fixture differs from checked manifest')
    destination = ROOT / 'fixtures/rtc_set'
    destination.mkdir(exist_ok=True)
    (destination / 'rtc_set.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'rtc_set.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('RTC Time Set fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
