#!/usr/bin/env python3
"""Build an independent GPL FreeDOS HDD shell control, with no game resources.

Run prepare_freedos.py first. Keep its complete corresponding sources/notices
with this generated image. The unmodified pinned HDD boot source is assembled by
NASM; the original MIT IPL and PROBE execute as guest code, never host DOS.
"""
import json
import sys
from build_media import ROOT, assemble
from prepare_freedos import extract_fat_file
from prepare_hdd_fixture import image_with_files, load_boot, sha


def main():
    dest = ROOT / 'fixtures/freedos'
    base = (dest / 'minimal.hdm').read_bytes()
    # Verify prepared inputs even when the caller reuses a local cache.
    expected = json.loads((dest / 'manifest.json').read_text())
    if sha(base) != expected['image_sha256']:
        raise ValueError('minimal.hdm checksum mismatch; rerun prepare_freedos.py')
    files = {name: extract_fat_file(base, name) for name in [
        b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']}
    files.update({
        b'FDCONFIGSYS': b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n',
        b'AUTOEXECBAT': b'@echo off\r\nPROMPT $p$g\r\necho HDD SHELL READY\r\n',
        b'PROBE   COM': assemble('guest/probe.asm'),
        b'PROBE   ASM': (ROOT / 'guest/probe.asm').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
    })
    boot, source = load_boot()
    image, ipl = image_with_files(boot, files)
    manifest = {
        'image': 'freedos/shell.hdi', 'image_sha256': sha(image),
        'license': 'GPL-2.0-or-later (boot/kernel/FreeCOM); MIT (IPL/PROBE/helpers)',
        'sources': 'Keep entire fixtures/freedos including matching kernel/FreeCOM sources and notices; not a Pages asset',
        'boot_source_sha256': sha(source),
        'boot_build': 'nasm -f bin -DNEC98HDD -DISFAT12 boot.asm',
        'ipl_sha256': sha(ipl),
        'ipl_source': {'path': 'guest/hdd-ipl.asm', 'license': 'MIT',
                       'sha256': sha((ROOT / 'guest/hdd-ipl.asm').read_bytes())},
        'geometry': [64, 8, 16, 512], 'header_bytes': 4096,
        'partition_lba': 128, 'partition_sectors': 8064,
        'fat': {'bits': 12, 'reserved': 2, 'copies': 2, 'sectors_per_fat': 6,
                'root_entries': 192, 'sectors_per_cluster': 4},
        'kernel_handoff': {'cs': 0x60, 'ip': 0, 'physical': 0x600},
        'slot_budget': 8000000,
        'files': {k.decode(): {'sha256': sha(v), 'bytes': len(v)} for k, v in files.items()},
    }
    text = json.dumps(manifest, indent=2) + '\n'
    pin = ROOT / 'fixtures/freedos-hdd-manifest.json'
    if '--update-manifest' in sys.argv:
        pin.write_text(text)
    elif pin.read_text() != text:
        raise ValueError('FreeDOS HDD differs from fixtures/freedos-hdd-manifest.json')
    (dest / 'shell.hdi').write_bytes(image)
    (dest / 'hdd-boot.asm').write_bytes(source)
    print('FreeDOS HDD verified:', sha(image))


if __name__ == '__main__':
    main()
