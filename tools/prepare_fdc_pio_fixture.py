#!/usr/bin/env python3
"""Build the original MIT non-DMA READ DATA probe offline; verify provenance."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def expected_results():
    result = bytearray([0xa5] * 64)
    result[1:9] = bytes([1, 0, 8, 8, 0x80, 0, 0x98, 1])
    result[16:23] = bytes([0x40, 0x80, 0, 0, 0, 3, 3])
    result[24:29] = bytes([0xd0, 0xd0, 0x80, 0, 0])
    return bytes(result)


def build(update=False):
    source = 'guest/fdc_pio.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = bytearray((i * 17 + i // 1024 * 23) & 255 for i in range(1_261_568))
    image[:1024] = boot
    payload = bytes((i ^ (i >> 8)) & 255 for i in range(2048))
    image[1024:3072] = payload
    firmware = assemble('guest/firmware.asm')
    result = expected_results()
    manifest = {
        'source': source,
        'license': 'MIT; original source, synthetic firmware source and LICENSE copied alongside image',
        'assembly': 'nasm -f bin guest/fdc_pio.asm -o fdc_pio.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/fdc_pio/fdc_pio.hdm',
        'image_sha256': sha(image),
        'firmware_source': 'guest/firmware.asm',
        'firmware_source_sha256': sha((ROOT / 'guest/firmware.asm').read_bytes()),
        'firmware_sha256': sha(firmware),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['VM', 'Ap'],
        'provenance': 'Original source-built bare-metal probe; no acquired binary, commercial ROM, DOS or copied emulator implementation',
        'contract': 'Raw SPECIFY ND=1 and MFM READ DATA sectors 2..3; masked channel 2 AND disabled DMA controller; actual INT 13h handler distinguishes EXM data and result requests, collects payload and results, reads cascade ISRs, EOIs both PICs; guest validates all payload/results/guards and SP before PASS',
        'specifications': [
            'NEC uPD765 specification, processor interface and READ DATA functional description; Table 2 applies only to processor/TC termination',
            'NEC uPD765A/B Tables 2-4: EXM/DIO/RQM/CB, ST0 abnormal, ST1 EN/OR, result order',
            'NEC Application Note 8, page 2: MFM normal 16 us byte interval, worst-case 13 us service time',
        ],
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8001=done; 8002 word=2048 data IRQs; 8004/5=slave/master ISR aggregates 08/80; 8006 word=9800 payload end; 8008=1 result IRQ; 8010..16=40 80 00 00 00 03 03; 8018..1c=MSR before/after first/after last result and post-EOI ISRs: d0 d0 80 00 00; remaining record bytes A5',
        'payload_address': '0x9000',
        'payload_length': len(payload),
        'payload_sha256': sha(payload),
        'payload_pattern': '(i ^ (i >> 8)) & 255 for i in range(2048)',
        'guards': 'Record A5 bytes, entire 8040..8fff and 9800..980f must remain A5; guest validates each; hosts compare records, complete payload, both adjacent 16-byte guards and unchanged full media',
        'termination': 'EOT without TC: abnormal ST0=40, ST1.EN=80, ST2=0, final sector ID 00 00 03 03; not the C+1/R=1 processor-termination row',
        'markers': ['FDC PIO PASS', 'FDC PIO FAIL'],
        'host': 'Mount, step, inspect and reset only; no host-supplied guest results or IRQs',
        'media': 'Complete unchanged exported floppy; zero writes',
        'slot_budget': 2_000_000,
        'reset': 'Same image, result, payload and guard expectations before/after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    checked = ROOT / 'fixtures/fdc-pio-manifest.json'
    if update:
        checked.write_text(text)
    elif checked.read_text() != text:
        raise ValueError('FDC PIO fixture differs from fixtures/fdc-pio-manifest.json')
    destination = ROOT / 'fixtures/fdc_pio'
    destination.mkdir(exist_ok=True)
    (destination / 'fdc_pio.hdm').write_bytes(image)
    for path in [source, 'guest/firmware.asm', 'LICENSE']:
        (destination / path.split('/')[-1]).write_bytes((ROOT / path).read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('FDC PIO fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
