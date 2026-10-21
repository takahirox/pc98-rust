#!/usr/bin/env python3
"""Reproduce the JIS X 0208 replacement CG from licensed Shinonome source."""
import gzip
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'fixtures/fonts/shinonome16.bit.gz'
OUTPUT = ROOT / 'fixtures/fonts/jis0208.bin'


def build():
    source = gzip.decompress(SOURCE.read_bytes())
    # Pin the original upstream bytes, independently of gzip metadata.
    assert hashlib.sha256(source).hexdigest() == '38ed485b3565b9c71147a459a9d0f8057714948ca6eaa2e6a3d74c95868d6467'
    font = bytearray(94 * 94 * 32)
    code = None
    rows = None
    for line in source.decode('utf-8').splitlines():
        if line.startswith('ENCODING '):
            code = int(line.split()[1])
        elif line == 'BITMAP':
            rows = []
        elif rows is not None and len(rows) < 16:
            assert len(line) == 16 and set(line) <= {'.', '@'}
            rows.append(int(line.replace('.', '0').replace('@', '1'), 2))
            if len(rows) == 16:
                row, cell = code >> 8, code & 255
                assert 0x21 <= row <= 0x7e and 0x21 <= cell <= 0x7e
                at = ((row - 0x21) * 94 + cell - 0x21) * 32
                font[at:at+32] = b''.join(v.to_bytes(2, 'big') for v in rows)
                rows = None
    return font


if __name__ == '__main__':
    data = build()
    if '--check' in sys.argv:
        assert OUTPUT.read_bytes() == data, 'stale replacement CG; run tools/build_font.py'
    else:
        OUTPUT.write_bytes(data)
    print(f'JIS CG: {len(data)} bytes, SHA-256 {hashlib.sha256(data).hexdigest()}')
