#!/usr/bin/env python3
"""Build the original MIT Ap RTC probe offline, outside publication assets."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def result(second):
    serial = bytes([second, 0, 0, 1, 0x14])
    bios = bytes([0x26, 0x14, 1, 0, 0, second])
    later = bytes([second + 1, 0, 0, 1, 0x14])
    return (b'\xa5' + serial + serial + b'\x26\x01' + bios
            + later + bios[:-1] + bytes([second + 1, 0x5a]) + b'\xa5' * 33)


def build(update=False):
    source = 'guest/rtc.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    assert len(result(0)) == 64
    manifest = {
        'source': source,
        'license': 'MIT; LICENSE; original pc98-rust contributors',
        'provenance': 'Original source-built probe; no commercial ROM, DOS or guest software',
        'assembler': 'nasm -f bin guest/rtc.asm -o rtc.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['Ap'],
        'initialization': 'Cold Machine::new: 2026-01-01 Thursday 00:00:00; no host time; reset preserves RTC and fractional clocks',
        'host': 'Insert identical image, step and read only; no result patches, clock injection or bypassed guest instructions',
        'contract': '40-bit parallel capture, 52-bit serial capture including command nibble, coherent BIOS INT/IRET, guest polling observes next second with IF clear, all date bytes/guards and port bits checked before PASS',
        'result_address': '0x8000',
        'result_length': 64,
        'result_hex': result(0).hex(),
        'reset_result_hex': result(1).hex(),
        'result_layout': '8000 guard; 8001..8005 parallel; 8006..800c extended (command nibble=1); 800d..8012 BIOS; 8013..8017 next parallel; 8018..801d next BIOS; 801e status=5a; 801f..803f guards=a5',
        'guards': '8000 and 801f..803f remain a5; guest and hosts assert all 64 result bytes',
        'markers': ['RTC CALENDAR PASS', 'RTC CALENDAR FAIL'],
        'slot_budget': 2_000_000,
        'reset': 'Same disk and budget on both boots; exact seconds 00->01 cold and 01->02 after reset',
        'distribution': 'Test only; generated fixtures/rtc excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/rtc-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('RTC fixture differs from fixtures/rtc-manifest.json')
    destination = ROOT / 'fixtures/rtc'
    destination.mkdir(exist_ok=True)
    (destination / 'rtc.hdm').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'rtc.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('RTC fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
