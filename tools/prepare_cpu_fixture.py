#!/usr/bin/env python3
"""Pin independent public-domain CPULEVEL and run it through guest FreeDOS.

The original archive (complete corresponding source and binary) remains beside
this optional disk. Nothing from here is published in web/. No emulator code is
acquired. Run prepare_freedos.py first.
"""
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

from prepare_dos_apps import DEST, fetch, sha
from prepare_freedos import ROOT, extract_fat_file, image_with_files

URL = ('https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/'
       'util/system/cpulevel/cpulevel2007.zip')
# Source header explicitly donates CPULEVEL to the public domain. Keep the source
# with the binary, as requested by the author (there is no separate README).
ARCHIVE_SHA = 'c7c8d820846af5c2f33b2679c55c435bb7d2067014da911c0bfd49e47e8d61d3'


def main():
    DEST.mkdir(parents=True, exist_ok=True)
    archive = fetch('cpulevel2007.zip', URL, ARCHIVE_SHA)
    with zipfile.ZipFile(io.BytesIO(archive)) as package:
        source = package.read('cpulevel.asm')
        binary = package.read('cpulevel.com')
    if not source.startswith(b'; CPULEVEL - This tool is hereby donated to the public domain.'):
        raise ValueError('missing CPULEVEL public-domain dedication')
    # Assemble the unmodified source as well as retaining the published UPX binary.
    # Native/browser acceptance executes this reproducible, uncompressed COM.
    with tempfile.TemporaryDirectory() as directory:
        src, dst = Path(directory) / 'cpulevel.asm', Path(directory) / 'cpulevel.com'
        src.write_bytes(source)
        subprocess.run(['nasm', '-f', 'bin', str(src), '-o', str(dst)], check=True)
        built = dst.read_bytes()
    base = (ROOT / 'fixtures/freedos/minimal.hdm').read_bytes()
    files = {
        name: extract_fat_file(base, name) for name in
        (b'KERNEL  SYS', b'COMMAND COM', b'FDCONFIGSYS', b'COPYING    ')
    }
    files.update({
        b'CPULEVELCOM': built,
        b'CPULEVELASM': source,
        b'AUTOEXECBAT': (b'@ECHO OFF\r\nCPULEVEL\r\n'
                          b'IF ERRORLEVEL 5 GOTO END\r\n'
                          b'IF ERRORLEVEL 4 ECHO CPULEVEL 486 VERIFIED\r\n'
                          b':END\r\n'),
    })
    image = image_with_files(base[:1024], files)
    manifest = {
        'name': 'CPULEVEL',
        'author': 'Eric Auer',
        'version': '2004-2008 / cpulevel2007.zip (2008-01-04 archive)',
        'source': {'url': URL, 'sha256': sha(archive)},
        'requirements': ['DOS INT 21h', '386 EAX/EBX/EBP/ESP operands',
                         'PUSHFD/POPFD', '486 EFLAGS AC; no CPUID or FPU'],
        'build': 'nasm -f bin cpulevel.asm -o cpulevel.com (unmodified source)',
        'rights': 'Public domain, explicit dedication at cpulevel.asm line 1; retain source with binary',
        'files': {
            'cpulevel.asm': {'sha256': sha(source), 'rights': 'Public domain, Eric Auer; exact archive member'},
            'cpulevel.com (archive)': {'sha256': sha(binary), 'rights': 'Public domain; keep cpulevel.asm'},
            'CPULEVEL.COM (assembled)': {'sha256': sha(built), 'rights': 'Public domain; unmodified corresponding source included'},
        },
        'dos': '../freedos/manifest.json (GPL-2.0-or-later, keep complete sources/notices)',
        'image_sha256': sha(image),
        'image_files': {name.decode(): sha(data) for name, data in files.items()},
        'expected': 'i486 or compatible; CPUID support check negative; DOS errorlevel 4 and CPULEVEL 486 VERIFIED',
        'limits': 'Portable DOS CPU diagnostic, not a PC-98 game or hardware-device acceptance fixture',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/cpu-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif not expected.exists() or expected.read_text() != text:
        raise ValueError('CPU fixture differs from audited fixtures/cpu-manifest.json')
    (DEST / 'cpu386.hdm').write_bytes(image)
    (DEST / 'cpulevel.asm').write_bytes(source)
    (DEST / 'cpu-manifest.json').write_text(text)
    print(f'CPU fixture verified: {sha(image)}; keep archive/source and fixtures/freedos with disk')


if __name__ == '__main__':
    main()
