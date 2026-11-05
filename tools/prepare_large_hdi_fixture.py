#!/usr/bin/env python3
"""Prepare original MIT large-HDI probe. Large containers are sparse, test-local.

Only this 8 KiB guest binary and its source/license are prepared, never gigabyte
fixture artifacts or private media. Native/browser tests create their own HDIs.
"""
import hashlib
import json
import sys
from build_media import ROOT, assemble


def main():
    binary = assemble('guest/large_hdi.asm')
    assert len(binary) == 8192
    manifest = {
        'source': 'guest/large_hdi.asm',
        'license': 'MIT; original implementation; no acquired guest/emulator code',
        'source_sha256': hashlib.sha256((ROOT / 'guest/large_hdi.asm').read_bytes()).hexdigest(),
        'binary_sha256': hashlib.sha256(binary).hexdigest(),
        'geometry': {'cylinders': [2048, 4096], 'heads': 16, 'sectors': 32, 'sector_bytes': 512, 'offset': 4096},
        'capacity_bytes': [512 * 1024**2, 1024**3],
        'checks': ['IDENTIFY/BIOS capacity', 'PIO two-sector writes/reads at low, >32MiB, middle, last',
                   'last-sector CHS/BIOS read/write', 'out-of-range PIO write/BIOS LBA',
                   'guest detects persisted last-sector data on reset/reimport'],
        'distribution': 'Test only; generate containers locally; excluded from web',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    pin = ROOT / 'fixtures/large-hdi-manifest.json'
    if '--update-manifest' in sys.argv:
        pin.write_text(text)
    elif pin.read_text() != text:
        raise ValueError('large HDI probe differs from pinned manifest')
    dest = ROOT / 'fixtures/large_hdi'
    dest.mkdir(exist_ok=True)
    (dest / 'probe.bin').write_bytes(binary)
    (dest / 'large_hdi.asm').write_bytes((ROOT / 'guest/large_hdi.asm').read_bytes())
    (dest / 'LICENSE').write_bytes((ROOT / 'LICENSE').read_bytes())
    print('Large HDI probe verified:', manifest['binary_sha256'])


if __name__ == '__main__':
    main()
