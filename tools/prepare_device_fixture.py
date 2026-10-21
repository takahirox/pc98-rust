#!/usr/bin/env python3
"""Independent DOS DEBUG exercises documented EGC and PEGC ports and VRAM.

The sole guest adaptation disables its optional x87 detection; CPU detection,
command parsing, DOS file I/O, port I/O and memory operations remain upstream.
"""
import io
import json
import sys
import zipfile
from build_media import assemble

from prepare_dos_apps import DEST, ROOT, fetch, sha
from prepare_freedos import extract_fat_file, image_with_files
from prepare_music_fixture import run

URL = 'https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/dos/debug/vojta/1.25/DEBUG125.zip'
HASH = '988360f4ddc02c3d7fbfa223c11d244c03bd54fc2435df4851ca9f6bdc0ed78c'


def build():
    data = fetch('debug125.zip', URL, HASH)
    archive = zipfile.ZipFile(io.BytesIO(data))
    directory = DEST / 'debug-build'
    directory.mkdir(exist_ok=True)
    for name in ['DEBUG.ASM', 'DEBUGTBL.INC', 'FPTOSTR.INC']:
        (directory / name.lower()).write_bytes(archive.read(name))
    source = archive.read('DEBUG.ASM')
    adapted = source.replace(b'init6:\r\n', b'init6:\r\n\tjmp init7 ; build without optional x87 detection\r\n')
    (directory / 'debug.asm').write_bytes(adapted)
    assembler = DEST / 'music-build/JWasm-a5c4ea03cc0545a15d81a354251b5f534bef7a1b/GccUnixR/jwasm'
    if not assembler.exists():
        raise RuntimeError('run python3 tools/prepare_music_fixture.py first')
    run([str(assembler), '-D?PM=0', '-bin', '-FoDEBUG.COM', 'debug.asm'], directory)
    commands = ['o 6a 7', 'o 6a 5', 'o 7c 80']
    for port, word in [(0x4a0, 0xfff0), (0x4a2, 0x40ff), (0x4a4, 0x2caa),
                       (0x4a6, 7), (0x4a8, 0), (0x4aa, 0), (0x4ac, 0), (0x4ae, 15)]:
        commands += [f'o {port:x} {word & 255:x}', f'o {port+1:x} {word >> 8:x}']
    commands += ['f a800:0 l10 ff']
    for plane in range(4):
        commands += [f'o 4a3 {0x40+plane:x}', f'm a800:0 l10 9000:{plane*16:x}']
    commands += ['o 7c 0', 'o 6a 4', 'o 6a 21', 'o 68 e', 'o a2 47', 'o a0 50', 'o a2 d']
    for i in range(256):
        commands += [f'o a8 {i:x}', f'o ac {i:x}', f'o aa {255-i:x}', f'o ae {i^85:x}']
    for bank in range(16):
        commands += [f'e e000:4 {bank:x} 0', f'f a800:0 l8000 {bank:x}']
    # 256 distinct colors in the first bank, then restore a readable bank.
    commands += ['e e000:4 0 0']
    for i in range(256):
        commands += [f'f a800:{i*128:x} l80 {i:x}']
    commands += ['q']
    base = (ROOT / 'fixtures/freedos/minimal.hdm').read_bytes()
    files = {n: extract_fat_file(base, n) for n in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ', b'FDCONFIGSYS']}
    files.update({b'DEBUG   COM': (directory / 'DEBUG.COM').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
        b'MEM98   COM': assemble('guest/memory98.asm'),
        b'DEBUG   ASM': source, b'DEBUG   TXT': archive.read('README.TXT'),
        b'PEGC    TXT': ('\r\n'.join(commands) + '\r\n').encode(),
        b'AUTOEXECBAT': b'@echo off\r\nMEM98\r\nDEBUG<PEGC.TXT>DEBUG.LOG\r\necho PEGC UTILITY COMPLETE\r\n'})
    image = image_with_files(base[:1024], files)
    (DEST / 'pegc.hdm').write_bytes(image)
    manifest = {'image_sha256': sha(image), 'utility': {'name': 'Paul Vojta / Japheth DEBUG 1.25',
        'url': URL, 'archive_sha256': HASH, 'license': 'MIT (Vojta), public domain (Japheth extensions)',
        'modification': 'Skip optional x87 detection, unsupported by this CPU profile',
        'source_sha256': sha(source), 'adapted_sha256': sha(adapted),
        'build': 'Pinned JWasm from music-manifest.json: -D?PM=0 -bin -FoDEBUG.COM debug.asm'},
        'requirements': ['Guest FreeDOS/FreeCOM', 'EGC aligned raster fill/read', 'PEGC packed-pixel 640x400',
                         'Two 32 KiB windows, sixteen banks, 256 x 24-bit palette', 'GDC PITCH'],
        'files': {n.decode(): {'sha256': sha(v), 'rights':
            'GPL-2.0-or-later' if n in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']
            else 'MIT/public-domain; original archive notices retained' if n.startswith(b'DEBUG')
            else 'MIT'} for n, v in files.items()},
        'original_probe': {'source': 'guest/memory98.asm', 'sha256': sha((ROOT / 'guest/memory98.asm').read_bytes()), 'license': 'MIT'},
        'redistribution': 'Keep original DEBUG125.zip (all source/notices), adapted source/build script, complete FreeDOS sources/notices and music-build compiler sources/notices. Outside Pages.'}
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/device-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('device fixture differs from fixtures/device-manifest.json')
    (DEST / 'device-manifest.json').write_text(text)
    print('Device fixture verified:', sha(image))


if __name__ == '__main__':
    build()
