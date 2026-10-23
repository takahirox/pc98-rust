#!/usr/bin/env python3
"""Build the original MIT IDE VERIFY HDI offline and check its provenance."""
import argparse
import hashlib
import json
import struct
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def expected_results():
    # Independent arithmetic oracle; never reads the guest's literal result table.
    def address(lba, mode):
        if mode == 'LBA':
            return [lba & 255, (lba >> 8) & 255, (lba >> 16) & 255, 0xe0 | (lba >> 24)]
        cylinder, track = divmod(lba, 128)
        head, sector = divmod(track, 32)
        return [sector + 1, cylinder & 255, cylinder >> 8, 0xa0 | head]

    cases = []
    for opcode in [0x40, 0x41]:
        for mode, scenarios in [
            ('LBA', [(7, 1), (255, 2), (7, 255), (7, 256), (510, 2),
                     (510, 4), (510, 256), (512, 256), (65536, 2), (7, 1)]),
            ('CHS', [(7, 1), (31, 2), (127, 2), (7, 255), (7, 256),
                     (510, 2), (510, 4), (510, 256),
                     ([0, 0, 0, 0xa0], 1), ([33, 0, 0, 0xa0], 2),
                     ([1, 0, 0, 0xa4], 255), ([1, 4, 0, 0xa0], 256), (0, 1)]),
        ]:
            for start, count in scenarios:
                if isinstance(start, list):
                    final, remaining = start, count
                else:
                    verified = min(count, max(0, 512 - start))
                    remaining = count - verified
                    final = address(start + verified - (1 if remaining == 0 else 0), mode)
                error = 0x10 if remaining else 0
                result = [remaining & 255, *final, error, 0x51 if error else 0x50, 0xff]
                cases.append({'opcode': hex(opcode), 'mode': mode, 'start': start,
                              'sectors': count, 'result_hex': bytes(result).hex()})
    result = bytearray([0xa5] * 1024)
    result[1:9] = bytes([2, 2, 2, 0, 0, 2, 0, 1])
    for i, case in enumerate(cases):
        result[16 + i * 8:24 + i * 8] = bytes.fromhex(case['result_hex'])
    return bytes(result), cases


def build(update=False):
    source = 'guest/ide_verify.asm'
    boot = assemble(source)
    assert len(boot) == 8192
    image = bytearray(4096 + 512 * 512)
    struct.pack_into('<IIIIII', image, 8, 4096, 512 * 512, 512, 32, 4, 4)
    image[4096:] = bytes([0x5a]) * (512 * 512)
    image[4096:4096 + len(boot)] = boot
    result, cases = expected_results()
    firmware = assemble('guest/firmware.asm')
    manifest = {
        'source': source,
        'license': 'MIT; original source and LICENSE copied alongside image',
        'assembly': 'nasm -f bin guest/ide_verify.asm -o ide_verify.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/ide_verify/ide_verify.hdi',
        'image_sha256': sha(image),
        'geometry': {'cylinders': 4, 'heads': 4, 'sectors': 32, 'sector_bytes': 512, 'offset': 4096},
        'profiles': ['Ap'],
        'provenance': 'Original MIT bare-metal probe and synthetic HDI; no acquired guest software, proprietary firmware or copied emulator implementation',
        'firmware_source': 'guest/firmware.asm (original MIT synthetic IPL)',
        'firmware_source_sha256': sha((ROOT / 'guest/firmware.asm').read_bytes()),
        'firmware_sha256': sha(firmware),
        'specifications': [
            'Fujitsu MHC2032AT disk product manual, READ VERIFY, printed page 5-22: selected legacy last-success/first-error address and inclusive residual',
            'T13/1321D revision 3, section 8.28: 40h/41h non-data, count zero=256, IDNF; normal outputs unspecified in that later draft',
        ],
        'host': 'Mount, reset, step and read outputs only; no guest result writes or IRQ injection',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8000=a5; 8001..8007=PIC IR1 observations (2,2,2,0,0,2,0); 8008=1 successful IDENTIFY/read recovery; 8009..800f=a5; 8010..817f=46 eight-byte records (count,S,CL,CH,DH,error,status,data=ff); 8180..83ff=a5 guards',
        'guards': 'Guest and hosts check every byte of the 1024-byte result/guard block; guest checks SP=7c00',
        'cases': cases,
        'irq_contract': 'Alternate status retains device latch; PIC poll drains then IR1 relatches. Primary status clears device latch but leaves PIC IRR. Poll/EOI then no retrigger. nIEN suppresses new delivery and clearing nIEN exposes retained latch.',
        'media': 'Unchanged image and zero writes; 17 PIO sector reads (16 boot sectors plus one guest recovery read); VERIFY transfers no data and never sets DRQ',
        'markers': ['IDE VERIFY PASS', 'IDE VERIFY FAIL'],
        'slot_budget': 500_000,
        'reset': 'Identical image, exact results/guards and budget before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/ide-verify-manifest.json'
    if update:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('IDE VERIFY fixture differs from fixtures/ide-verify-manifest.json')
    destination = ROOT / 'fixtures/ide_verify'
    destination.mkdir(exist_ok=True)
    (destination / 'ide_verify.hdi').write_bytes(image)
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'ide_verify.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'firmware.asm').write_bytes((ROOT / 'guest/firmware.asm').read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('IDE VERIFY fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
