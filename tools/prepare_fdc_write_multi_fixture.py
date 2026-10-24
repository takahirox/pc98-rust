#!/usr/bin/env python3
"""Build the original MIT non-DMA multi-sector WRITE DATA/readback probe offline; verify provenance."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def expected_results():
    result = bytearray([0xa5] * 128)
    for base, end in [(0, 0x98), (64, 0xa0)]:
        result[base+1:base+9] = bytes([1, 0, 8, 8, 0x80, 0, end, 1])
        result[base+16:base+23] = bytes([0x40, 0x80, 0, 0, 0, 3, 3])
        result[base+24:base+29] = bytes([0xd0, 0xd0, 0x80, 0, 0])
    return bytes(result)


def build(update=False):
    source = 'guest/fdc_write_multi.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = bytearray((i * 17 + i // 1024 * 23) & 255 for i in range(1_261_568))
    image[:1024] = boot
    payload = bytes((29 * i + 71 + i // 1024 * 91) & 255 for i in range(2048))
    expected_media = image.copy()
    expected_media[1024:3072] = payload
    firmware = assemble('guest/firmware.asm')
    result = expected_results()
    manifest = {
        'source': source,
        'license': 'MIT; original source, synthetic firmware source and LICENSE copied alongside image',
        'license_sha256': sha((ROOT / 'LICENSE').read_bytes()),
        'assembly': 'nasm -f bin guest/fdc_write_multi.asm -o fdc_write_multi.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/fdc_write_multi/fdc_write_multi.hdm',
        'image_sha256': sha(image),
        'expected_media_sha256': sha(expected_media),
        'firmware_source': 'guest/firmware.asm',
        'firmware_source_sha256': sha((ROOT / 'guest/firmware.asm').read_bytes()),
        'firmware_sha256': sha(firmware),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap'],
        'provenance': 'Original source-built bare-metal probe; no acquired binary, commercial ROM, DOS or copied emulator implementation',
        'contract': 'Guest SPECIFY ND=1, single-head multi-sector MFM WRITE DATA R=2/EOT=3 with DTL=1, then PIO READ DATA readback; DMA masked AND controller disabled; actual INT 13h handlers supply/collect all bytes, record real PIC ISRs, EOI both PICs; guest checks both results, source/readback/guards and SP before PASS',
        'specifications': [
            'NEC uPD765 specification, processor interface and READ DATA functional description; Table 2 applies only to processor/TC termination',
            'NEC uPD765A/B Tables 2-4: EXM/DIO/RQM/CB, ST0 abnormal, ST1 EN/OR, result order',
            'NEC Application Note 8, page 2: MFM normal 16 us byte interval, worst-case 13 us service time',
        ],
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': 'Two 64-byte records at 8000/8040: +1=done; +2 word=2048 byte IRQs; +4/5=slave/master ISR aggregates 08/80; +6 word=9800/a000 end pointer; +8=1 separate result IRQ; +10h..16h=40 80 00 00 00 03 03; +18h..1ch=MSR before/after first/after last result and post-EOI ISRs d0 d0 80 00 00; padding A5',
        'payload_address': '0x9000',
        'payload_length': len(payload),
        'readback_address': '0x9800',
        'payload_sha256': sha(payload),
        'payload_pattern': '(29 * i + 71 + i // 1024 * 91) & 255 for i in range(2048)',
        'guards': 'Record A5 padding, entire 8080..8fff and a000..a00f remain A5; guest and hosts compare all guards, source 9000..97ff and readback 9800..9fff',
        'termination': 'EOT without TC: abnormal ST0=40, ST1.EN=80, ST2=0, final sector ID 00 00 03 03; not the C+1/R=1 processor-termination row',
        'markers': ['FDC WRITE MULTI PASS', 'FDC WRITE MULTI FAIL'],
        'host': 'Mount, step, inspect and reset only; no host-supplied guest results or IRQs',
        'media': 'Only bytes 1024..3071 become the independent payload; two sector writes per boot; all other image bytes unchanged',
        'slot_budget': 2_000_000,
        'reset': 'Identical image/result/source/readback/guards before/after reset; media retains the first boot write, guest writes same payload again',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    checked = ROOT / 'fixtures/fdc-write-multi-manifest.json'
    if update:
        checked.write_text(text)
    elif checked.read_text() != text:
        raise ValueError('FDC WRITE MULTI fixture differs from fixtures/fdc-write-multi-manifest.json')
    destination = ROOT / 'fixtures/fdc_write_multi'
    destination.mkdir(exist_ok=True)
    (destination / 'fdc_write_multi.hdm').write_bytes(image)
    for path in [source, 'guest/firmware.asm', 'LICENSE']:
        (destination / path.split('/')[-1]).write_bytes((ROOT / path).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('FDC WRITE MULTI fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
