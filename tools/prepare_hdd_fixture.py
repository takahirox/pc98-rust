#!/usr/bin/env python3
"""Reproduce a licensed PC-98 FreeDOS IDE installation, never emulator code.

Requires the existing FreeDOS and independent DOS application preparation.
All outputs stay beside their notices and complete corresponding sources.
"""
import hashlib
import json
import struct
import subprocess
import sys
import tarfile
import tempfile

from build_media import ROOT, assemble
from prepare_freedos import extract_fat_file

DEST = ROOT / 'fixtures/dos-apps'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def build():
    base = (ROOT / 'fixtures/freedos/minimal.hdm').read_bytes()
    adv = (DEST / 'adv98.hdm').read_bytes()
    # Pinned GPL source is assembled unchanged; retain it and all DOS sources.
    archive = ROOT / 'fixtures/freedos/kernel-source.tar.gz'
    with tarfile.open(archive) as tar:
        member = next(m for m in tar if m.name.endswith('/nec98/boot/boot.asm'))
        source = tar.extractfile(member).read()
    with tempfile.TemporaryDirectory() as td:
        from pathlib import Path
        asm, binary = Path(td) / 'boot.asm', Path(td) / 'boot.bin'
        asm.write_bytes(source)
        subprocess.run(['nasm', '-f', 'bin', '-DNEC98HDD', '-DISFAT12', str(asm), '-o', str(binary)], check=True)
        boot = binary.read_bytes()
    assert len(boot) == 512
    files = {name: extract_fat_file(base, name) for name in [
        b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']}
    files.update({name: extract_fat_file(adv, name) for name in [
        b'ADV98   EXE', b'SCRIPT  TXT', b'ADV     PAL', b'CLASS   G98', b'C01_N   SPR', b'APP     LIC']})
    files[b'FDCONFIGSYS'] = b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n'
    files[b'MOUSE98 COM'] = assemble('guest/mouse98.asm')
    files[b'RUST    LIC'] = (ROOT / 'LICENSE').read_bytes()
    files[b'AUTOEXECBAT'] = b'@echo off\r\necho HDD DOS BOOT\r\necho DURABLE DOS WRITE>HDDSAVE.TXT\r\nMOUSE98\r\nADV98\r\n'
    # 64 cylinders x 8 heads x 16 sectors. One DOS FAT12 partition at C=1.
    cylinders, heads, sectors = 64, 8, 16
    hidden, total, reserved, fatsecs, roots, spc = 128, 8064, 2, 6, 192, 4
    rootsecs = roots * 32 // 512
    datastart = reserved + fatsecs * 2 + rootsecs
    volume = bytearray(total * 512)
    volume[:512] = boot
    struct.pack_into('<HBHBHHBHHHII', volume, 11, 512, spc, reserved, 2, roots,
                     total, 0xf8, fatsecs, sectors, heads, hidden, 0)
    volume[36:62] = b'\x80\x00\x29' + struct.pack('<I', 0x98524453) + b'FREEDOS HDD' + b'FAT12   '
    struct.pack_into('<IHHBIHIIHH', volume, 62, hidden, datastart, 512, 0,
                     reserved + fatsecs * 2, rootsecs, reserved, datastart, 0, 0x60)
    fat = bytearray(fatsecs * 512)
    fat[:3] = b'\xf8\xff\xff'
    cluster = 2
    for index, (name, data) in enumerate(files.items()):
        count = (len(data) + spc * 512 - 1) // (spc * 512)
        assert cluster + count < 0xff0
        entry = bytearray(32)
        entry[:11] = name
        entry[11] = 0x20
        struct.pack_into('<H', entry, 26, cluster)
        struct.pack_into('<I', entry, 28, len(data))
        at = (reserved + fatsecs * 2) * 512 + index * 32
        volume[at:at+32] = entry
        for i in range(count):
            c = cluster + i
            nxt = c + 1 if i + 1 < count else 0xfff
            off = c * 3 // 2
            word = int.from_bytes(fat[off:off+2], 'little')
            word = (word & 15) | (nxt << 4) if c & 1 else (word & 0xf000) | nxt
            fat[off:off+2] = word.to_bytes(2, 'little')
        at = (datastart + (cluster - 2) * spc) * 512
        volume[at:at+len(data)] = data
        cluster += count
    volume[reserved*512:(reserved+fatsecs)*512] = fat
    volume[(reserved+fatsecs)*512:(reserved+2*fatsecs)*512] = fat
    image = bytearray(4096 + cylinders * heads * sectors * 512)
    struct.pack_into('<IIIIII', image, 8, 4096, len(image) - 4096, 512, sectors, heads, cylinders)
    ipl = assemble('guest/hdd-ipl.asm')
    image[4096:4096+512] = ipl
    part = bytearray(32)
    part[:2] = b'\xa1\x81'
    struct.pack_into('<BBH', part, 4, 0, 0, 1)  # IPL CHS
    struct.pack_into('<BBH', part, 8, 0, 0, 1)  # partition first CHS
    struct.pack_into('<BBH', part, 12, 15, 7, 63)  # inclusive last CHS
    part[16:] = b'FreeDOS IDE     '
    image[4096+512:4096+544] = part
    image[4096+hidden*512:] = volume
    (DEST / 'adv98.hdi').write_bytes(image)
    (DEST / 'hdd-boot.asm').write_bytes(source)
    manifest = {
        'image': 'dos-apps/adv98.hdi', 'image_sha256': sha(image),
        'geometry': {'cylinders': cylinders, 'heads': heads, 'sectors': sectors, 'sector_bytes': 512},
        'requirements': ['PC-9821 IDE PIO', 'SASI-compatible INT 1B', 'PC-98 partition IPL', 'FreeDOS FAT12', 'ADV98'],
        'sources': ['freedos/manifest.json', 'dos-apps-manifest.json'],
        'boot_source': {'path': 'dos-apps/hdd-boot.asm', 'sha256': sha(source),
                        'license': 'GPL-2.0-or-later', 'build': 'nasm -f bin -DNEC98HDD -DISFAT12 boot.asm'},
        'ipl': {'source': 'guest/hdd-ipl.asm', 'sha256': sha(ipl), 'license': 'MIT'},
        'guest_sources': {name: {'sha256': sha((ROOT / name).read_bytes()), 'license': 'MIT'}
                          for name in ['guest/hdd-ipl.asm', 'guest/mouse98.asm']},
        'files': {k.decode(): {'sha256': sha(v), 'license':
            'GPL-2.0-or-later' if k in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    '] else 'MIT'}
                  for k, v in files.items()},
        'redistribution': 'Keep complete fixtures/freedos and fixtures/dos-apps outputs, source archives, notices and preparation scripts; never publish this GPL image alone.',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/hdd-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('HDD fixture differs from fixtures/hdd-manifest.json')
    (DEST / 'hdd-manifest.json').write_text(text)
    print('HDD fixture verified:', sha(image))


if __name__ == '__main__':
    build()
