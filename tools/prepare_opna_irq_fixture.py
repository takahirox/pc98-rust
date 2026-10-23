#!/usr/bin/env python3
"""Build the original MIT YM2608 IRQ probe offline and check provenance."""
import argparse
import hashlib
import json
from build_media import ROOT, assemble


def sha(data):
    return hashlib.sha256(data).hexdigest()


def expected_results():
    # Hardware contract oracle, independent of the assembly's expected table.
    result = bytearray([0xa5] * 128)
    result[1] = 4  # two actual vector-1Ch entries per timer
    records = []
    for timer, flag in enumerate([1, 2]):
        record = [flag, flag, 0, 16, 16, flag, 16, 128, 2 * timer + 1,
                  0, 0, flag, 16, 128, 2 * timer + 2, 0]
        result[16 + timer * 16:32 + timer * 16] = bytes(record)
        records.append({'timer': 'AB'[timer], 'result_hex': bytes(record).hex()})
    result[64:72] = bytes([0, 0, 0x87, 0x84] * 2)
    return bytes(result), records


def build(update=False):
    source = 'guest/opna_irq.asm'
    boot = assemble(source)
    assert len(boot) == 1024
    image = boot + bytes(1_261_568 - len(boot))
    result, records = expected_results()
    firmware = assemble('guest/firmware.asm')
    manifest = {
        'source': source,
        'license': 'MIT; original source and LICENSE copied alongside image',
        'assembly': 'nasm -f bin guest/opna_irq.asm -o opna_irq.bin',
        'source_sha256': sha((ROOT / source).read_bytes()),
        'boot_sha256': sha(boot),
        'image': 'fixtures/opna_irq/opna_irq.hdm',
        'image_sha256': sha(image),
        'geometry': '77 cylinders, 2 heads, 8 sectors, 1024 bytes per sector',
        'profiles': ['Ap'],
        'provenance': 'Original MIT bare-metal probe and synthetic floppy; no acquired binaries, proprietary firmware, samples or emulator implementation',
        'firmware_source': 'guest/firmware.asm (original MIT synthetic IPL)',
        'firmware_source_sha256': sha((ROOT / 'guest/firmware.asm').read_bytes()),
        'firmware_sha256': sha(firmware),
        'specifications': [
            'Yamaha YM2608 Application Manual sections 2-2-1 (page 19) and 2-8-3 (page 36)',
            'Intel 8259A datasheet: interrupt acknowledgement, IRR/ISR and normal EOI',
        ],
        'host': 'Mount, step, reset and read outputs only; no flag, interrupt or result injection',
        'result_address': '0x8000',
        'result_length': len(result),
        'result_hex': result.hex(),
        'result_layout': '8001=4 actual vector-1Ch handler entries; 8010..801f=Timer A, 8020..802f=Timer B; 8040..8047=master/slave poll acknowledgements (00,00 suppressed; 87,84 enabled, per timer); all other bytes A5',
        'record_layout': ['suppressed status', 'repeated status', 'suppressed IRR4',
                          'enabled pending IRR4', 'device-suppressed retained IRR4',
                          'handler status', 'handler slave ISR', 'handler master ISR',
                          'cumulative handler count', 'post-EOI slave ISR', 'post-EOI IRR4',
                          'recovery handler status', 'recovery slave ISR', 'recovery master ISR',
                          'cumulative handler count', 'recovery post-EOI IRR4'],
        'records': records,
        'guards': '8000,8002..800f,8030..803f,8048..807f remain A5; guest and hosts compare all 128 bytes; guest checks SP=7c00',
        'irq_contract': 'Each timer overflows while suppressed, status remains readable and PIC clean; enabling pending flag latches slave IR4; suppression retains request; actual vector-1Ch handler observes ISR4/master ISR7, clears flags with 27h, EOIs both; new overflow and enable recovers with output enabled at CPU entry; handler suppresses/clears the source and drains the scheduler-retained request with PIC poll acknowledgement and EOIs. Guest explicitly sets slave vector base=18h.',
        'markers': ['OPNA IRQ PASS', 'OPNA IRQ FAIL'],
        'media': 'Unchanged floppy; no guest disk writes',
        'slot_budget': 500_000,
        'reset': 'Identical image, results/guards and count before and after Machine reset',
        'distribution': 'Test only; excluded from web/',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    checked = ROOT / 'fixtures/opna-irq-manifest.json'
    if update:
        checked.write_text(text)
    elif checked.read_text() != text:
        raise ValueError('OPNA IRQ fixture differs from fixtures/opna-irq-manifest.json')
    destination = ROOT / 'fixtures/opna_irq'
    destination.mkdir(exist_ok=True)
    (destination / 'opna_irq.hdm').write_bytes(image)
    (destination / 'opna_irq.asm').write_bytes((ROOT / source).read_bytes())
    (destination / 'firmware.asm').write_bytes((ROOT / 'guest/firmware.asm').read_bytes())
    (destination / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    (destination / 'manifest.json').write_text(text)
    print('OPNA IRQ fixture verified:', sha(image))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--update-manifest', action='store_true')
    build(parser.parse_args().update_manifest)
