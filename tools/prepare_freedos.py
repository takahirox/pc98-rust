#!/usr/bin/env python3
"""Download pinned GPL guests + corresponding sources; make a minimal test disk.

No existing emulator implementation is downloaded or linked. All guest binaries
remain guests. Preserve the entire output folder if redistributing the image.
"""
import hashlib
import io
import json
from pathlib import Path
import sys
import tarfile
import urllib.request
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_media import ROOT, assemble

DEST = ROOT / 'fixtures/freedos'
ASSETS = {
    'distribution.zip': (
        'https://github.com/lpproj/fdkernel/releases/download/test-20220120-cherrypick/fd98_2hd144_20220123.zip',
        'ad12a889bae25dde5be566fccc44188f21762e241b7ff14ced3d2eac5114f5fa'),
    'freecom.zip': (
        'https://github.com/lpproj/freecom_dbcs2/releases/download/dbcswip-20220117/freecom_dbcswip-20220117-bin.zip',
        'f219de9fdb3f543e8e71f216d37d4aa231f77a04e95c7d804dc976ba9a994a0e'),
    'kernel-source.tar.gz': (
        'https://codeload.github.com/lpproj/fdkernel/tar.gz/679eda7eeeac6cc68bad4d3ff9b055e672529731',
        '92696b9bc23355a6ef2ba28ab87ef8f086faec509d05b2a499caa4116ed6fba3'),
    'freecom-source.tar.gz': (
        'https://codeload.github.com/lpproj/freecom_dbcs2/tar.gz/86a1a89eadbc14d8ff3f52e62dc51e98210de216',
        'e04a33594b8ef7347ae8fef96b6071d04985f32fabec4b0de3bbe0fbad9e5528'),
}

def fetch(name, url, sha):
    p = DEST / name
    b = p.read_bytes() if p.exists() else urllib.request.urlopen(url, timeout=60).read()
    if hashlib.sha256(b).hexdigest() != sha:
        raise ValueError(f'checksum mismatch: {name}')
    p.write_bytes(b)
    return b

def extract_fat_file(image, name):
    # This pinned distribution is 1024-byte sectors, one sector per cluster.
    root = image[5*1024:11*1024]
    entry = next(root[i:i+32] for i in range(0, len(root), 32) if root[i:i+11] == name)
    size = int.from_bytes(entry[28:32], 'little')
    cluster = int.from_bytes(entry[26:28], 'little')
    fat = image[1024:3072]
    result = bytearray()
    seen = set()
    while cluster < 0xff8:
        if cluster < 2 or cluster in seen:
            raise ValueError('invalid source FAT chain')
        seen.add(cluster)
        result += image[(11+cluster-2)*1024:(12+cluster-2)*1024]
        off = cluster*3//2
        cluster = int.from_bytes(fat[off:off+2], 'little') >> (4 if cluster&1 else 0) & 0xfff
    return bytes(result[:size])

def image_with_files(boot, files):
    image = bytearray(1261568)
    image[:1024] = boot
    fat = bytearray(2048)
    fat[:3] = bytes([0xfe, 0xff, 0xff])
    cluster = 2
    for idx, (name, data) in enumerate(files.items()):
        entry = bytearray(32)
        entry[:11] = name
        entry[11] = 0x20
        entry[26:28] = cluster.to_bytes(2, 'little')
        entry[28:32] = len(data).to_bytes(4, 'little')
        image[5*1024+idx*32:5*1024+(idx+1)*32] = entry
        count = (len(data)+1023)//1024
        for i in range(count):
            c = cluster+i
            nxt = c+1 if i+1<count else 0xfff
            off = c*3//2
            word = int.from_bytes(fat[off:off+2], 'little')
            word = (word&0x000f)|(nxt<<4) if c&1 else (word&0xf000)|nxt
            fat[off:off+2] = word.to_bytes(2, 'little')
            payload = data[i*1024:(i+1)*1024]
            at = (11+c-2)*1024
            image[at:at+len(payload)] = payload
        cluster += count
    image[1024:3072] = fat
    image[3072:5120] = fat
    return bytes(image)

def main():
    DEST.mkdir(parents=True, exist_ok=True)
    blobs = {name: fetch(name, *asset) for name, asset in ASSETS.items()}
    distribution = zipfile.ZipFile(io.BytesIO(blobs['distribution.zip']))
    original = distribution.read('fd98_2hd.img')
    shell = zipfile.ZipFile(io.BytesIO(blobs['freecom.zip']))
    # Copy neither the unrelated utilities nor their filesystem data/slack.
    files = {
        b'KERNEL  SYS': extract_fat_file(original, b'KERNEL  SYS'),
        b'COMMAND COM': shell.read('nec98/japanese/command.com'),
        b'PROBE   COM': assemble('guest/probe.asm'),
        b'FDCONFIGSYS': b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n',
        b'AUTOEXECBAT': b'@echo off\r\nPROMPT $p$g\r\n',
        b'COPYING    ': extract_fat_file(original, b'COPYING    '),
    }
    image = image_with_files(original[:1024], files)
    (DEST / 'minimal.hdm').write_bytes(image)
    # Separate original timer control: keep the established minimal image stable.
    timer_files = {k: v for k, v in files.items() if k != b'PROBE   COM'}
    timer_files.update({
        b'TIMER   COM': assemble('guest/timer.asm'),
        b'TIMER   ASM': (ROOT / 'guest/timer.asm').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
        b'AUTOEXECBAT': b'@echo off\r\nTIMER\r\n',
    })
    timer_image = image_with_files(original[:1024], timer_files)
    (DEST / 'timer.hdm').write_bytes(timer_image)
    (DEST / 'timer-manifest.json').write_text(json.dumps({
        'license': 'GPL-2.0-or-later (DOS); MIT (original TIMER source/COM)',
        'image_sha256': hashlib.sha256(timer_image).hexdigest(),
        'files': {k.decode(): hashlib.sha256(v).hexdigest() for k, v in timer_files.items()},
        'sources_and_notices': 'Keep this entire freedos folder, as for minimal.hdm',
    }, indent=2) + '\n')
    (DEST / 'COPYING').write_bytes(files[b'COPYING    '])
    (DEST / 'FreeCOM-LICENSE').write_bytes(shell.read('license'))
    (DEST / 'distribution-READMEja.htm').write_bytes(distribution.read('READMEja.htm'))
    manifest = {
        'license': 'GPL-2.0-or-later (boot, kernel, FreeCOM); MIT (PROBE.COM)',
        'image_sha256': hashlib.sha256(image).hexdigest(),
        'assets': {k: {'url': v[0], 'sha256': v[1]} for k, v in ASSETS.items()},
        'files': {k.decode('ascii'): hashlib.sha256(v).hexdigest() for k, v in files.items()},
    }
    (DEST / 'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    (DEST / 'README.txt').write_text(
        'Minimal PC-98 FreeDOS compatibility fixture. GPLv2 or later.\n'
        'Kernel/boot: lpproj/fdkernel test-20220120-cherrypick (679eda7).\n'
        'FreeCOM: lpproj/freecom_dbcs2 dbcswip-20220117 (86a1a89).\n'
        'PROBE.COM: original MIT guest, source in guest/probe.asm.\n'
        'timer.hdm: same DOS, original MIT guest/timer.asm + TIMER.COM/RUST.LIC.\n'
        'timer-manifest.json records its files; same source/license obligations.\n'
        'Configuration and AUTOEXEC generated by tools/prepare_freedos.py.\n'
        'No changes to kernel/boot/FreeCOM; the disk filesystem was rebuilt.\n'
        'See COPYING, FreeCOM-LICENSE, manifest.json, and the complete matching\n'
        'kernel-source.tar.gz and freecom-source.tar.gz (including build scripts).\n'
        'Redistribute this entire folder with the image to preserve sources,\n'
        'licenses, original notices and upstream distribution documentation.\n')
    # Verify source archives are valid without extracting paths from downloads.
    for name in ['kernel-source.tar.gz', 'freecom-source.tar.gz']:
        with tarfile.open(DEST / name) as tar:
            assert any(m.name.endswith('/makefile') or m.name.endswith('/makefile.mak') for m in tar)
    print(json.dumps(manifest, indent=2))

if __name__ == '__main__':
    main()
