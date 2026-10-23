#!/usr/bin/env python3
"""Prepare independently authored, MIT DOS guests, with complete provenance.

Only guest programs are downloaded, never emulator source. The ADV98 release's
artwork/music are excluded: use its MIT source scenario and original test art.
Preserve this entire directory and fixtures/freedos if redistributing these disks.
"""
import hashlib
import io
import json
from pathlib import Path
import struct
import sys
import tarfile
import urllib.request
import zipfile

from prepare_freedos import ROOT, extract_fat_file, image_with_files

DEST = ROOT / 'fixtures/dos-apps'
ASSETS = {
    'adv98.zip': (
        'https://github.com/yarufu/pc98/releases/download/v0.1/ADV98.zip',
        '9aa590f53b331ec04fa1a2352f7ed6190407756d1f552c9c0bf76f9a05eb2d3d'),
    'adv98-engine-source.tar.gz': (
        'https://codeload.github.com/yarufu/pc98/tar.gz/11a9f1c59c6fd8169439d477d5276ee7c0f27566',
        'cd5fb3506518c959c625dbdb62b08e9b60fa1548f7f8e6a8a7a6a24943738977'),
    'adv98-scenario-source.tar.gz': (
        'https://codeload.github.com/yarufu/pc98/tar.gz/2f482577433f89abd3166e95209f3ffea21b9bbc',
        '79533a772499fc486cde2f32292a7389d9be53458c6843e1782050b4a1413a3f'),
    '98videop.com': (
        'https://github.com/maxotaku11niku/98VIDEOP/releases/download/v1.4.0/98VIDEOP.COM',
        '877a0086d16da70e8166d41f2ff6043269a6f6235c0aba9e4d818129b74cbb99'),
    '98videop-source.tar.gz': (
        'https://codeload.github.com/maxotaku11niku/98VIDEOP/tar.gz/c46b5f3cdcae83c81cf829adb900da400357bbc0',
        '0bff1983e0560be65e110851b87b0e6f82405e46fe893a1b4bdfa6b83476e68d'),
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetch(name, url, checksum, *, fallback_urls=()):
    path = DEST / name

    def verify(data):
        if len(data) > 8 * 1024 * 1024 or sha(data) != checksum:
            raise ValueError(f'checksum or size mismatch: {name}')
        return data

    if path.exists():
        return verify(path.read_bytes())
    urls = (url, *fallback_urls)
    for index, candidate in enumerate(urls):
        try:
            with urllib.request.urlopen(candidate, timeout=60) as response:
                data = verify(response.read(8 * 1024 * 1024 + 1))
        except (OSError, ValueError) as error:
            print(f'Failed to fetch {name} from {candidate}: {error}', file=sys.stderr)
            if index == len(urls) - 1:
                raise
        else:
            # Cache only bytes verified against the same pin at every mirror.
            path.write_bytes(data)
            return data


def source_file(data, name):
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        entries = [m for m in archive if m.name.split('/', 1)[-1] == name]
        if len(entries) != 1 or not entries[0].isfile():
            raise ValueError(f'missing/ambiguous source file: {name}')
        return archive.extractfile(entries[0]).read()


def video():
    # Original MIT test data for the public 98V format, no third-party video.
    # Eight frames alternate guest-decoded fills in all four planes.
    nibbles = [level for i in range(16) for level in (
        15 if i & 2 else (7 if i & 8 else 0),
        15 if i & 4 else (7 if i & 8 else 0),
        15 if i & 1 else (7 if i & 8 else 0))]
    palette = bytes((nibbles[i] << 4) | nibbles[i+1] for i in range(0, 48, 2))
    data = bytearray(b'98V' + struct.pack('<BIH', 3, 8, 15) + palette)
    for frame in range(8):
        data += struct.pack('<H', 32) + b'\x20\x60' * 32  # buzzer PCM
        data += struct.pack('<H', 15)
        for plane in range(4):
            fill = 0xffff if (frame + plane) & 1 else 0
            chunk = struct.pack('<HHHHH', 1, 0, 16000, fill, 0)
            data += struct.pack('<H', len(chunk) // 2) + chunk
    return bytes(data)


def background():
    # G98B: 13-byte header, followed by B/R/G/I planar pixels, MSB first.
    data = bytearray(b'G98B' + struct.pack('<HH', 640, 400) + b'\x01BRGI')
    for plane in range(4):
        for y in range(400):
            for byte_x in range(80):
                color = (byte_x // 5 + y // 50) & 15
                data.append(255 if color & (1 << plane) else 0)
    return bytes(data)


def sprite():
    # Original geometric marker for the source scenario's optional stand image.
    return b'SPR0' + struct.pack('<HHB', 32, 64, 1) + bytes(
        7 if 8 <= x < 24 and 8 <= y < 56 else 0
        for y in range(64) for x in range(32))


def main():
    DEST.mkdir(parents=True, exist_ok=True)
    blobs = {n: fetch(n, *asset) for n, asset in ASSETS.items()}
    for name in ['fm86.c', 'fm86.h']:
        if (ROOT / 'fixtures/fm98' / name).read_bytes() != source_file(
                blobs['adv98-scenario-source.tar.gz'], f'src/{name}'):
            raise ValueError(f'copied independent FM source differs: {name}')
    if sha((ROOT / 'fixtures/fm98/fm98.com').read_bytes()) != '0ff73bd58e4293137738e66b1bb67e324f24327ff370581243f1815f5beb28ea':
        raise ValueError('FM guest differs; rebuild with tools/build_fm_test.py and audit provenance')
    base = (ROOT / 'fixtures/freedos/minimal.hdm').read_bytes()
    common = {n: extract_fat_file(base, n) for n in [
        b'KERNEL  SYS', b'COMMAND COM', b'FDCONFIGSYS', b'COPYING    ']}
    scenario = source_file(blobs['adv98-scenario-source.tar.gz'], 'script_サンプル2.txt')
    adv_files = {
        b'ADV98   EXE': zipfile.ZipFile(io.BytesIO(blobs['adv98.zip'])).read('ADV98.EXE'),
        b'SCRIPT  TXT': scenario,
        b'ADV     PAL': source_file(blobs['adv98-scenario-source.tar.gz'], 'adv.pal'),
        b'CLASS   G98': background(),
        b'C01_N   SPR': sprite(),
        b'AUTOEXECBAT': b'@echo off\r\nADV98\r\n',
    }
    video_files = {
        b'98VIDEOPCOM': blobs['98videop.com'],
        b'TEST    98V': video(),
        b'AUTOEXECBAT': b'@echo off\r\n98VIDEOP TEST.98V\r\necho VIDEO RETURNED\r\n',
    }
    fm_files = {
        b'FM98    COM': (ROOT / 'fixtures/fm98/fm98.com').read_bytes(),
        b'AUTOEXECBAT': b'@echo off\r\nFM98\r\n',
    }
    manifests = {}
    for name, files, source in [
        ('adv98', adv_files, 'adv98-engine-source.tar.gz'),
        ('98videop', video_files, '98videop-source.tar.gz'),
        ('fm98', fm_files, 'adv98-scenario-source.tar.gz'),
    ]:
        license_data = source_file(blobs[source], 'LICENSE')
        (DEST / f'{name}-LICENSE').write_bytes(license_data)
        image = image_with_files(base[:1024], {**common, **files, b'APP     LIC': license_data})
        (DEST / f'{name}.hdm').write_bytes(image)
        manifests[name] = {
            'license': 'MIT (application); GPL-2.0-or-later (guest DOS)',
            'image_sha256': sha(image),
            'files': {k.decode('ascii'): sha(v) for k, v in files.items()},
        }
    manifests['adv98'].update({
        'version': 'v0.1 engine, 11a9f1c; sample 2 from 2f482577',
        'requirements': 'PC-98 DOS, conventional RAM, 640x400 four-plane graphics, CG, keyboard, writable floppy',
        'expected': 'Japanese title/dialogue, Look/Talk/Move branches, SAVE1.DAT save/load',
        'modifications': 'Unmodified executable/scenario; original replacement G98B background and SPR0 marker; optional PMD/mouse absent',
    })
    manifests['98videop'].update({
        'version': 'v1.4.0, c46b5f3c',
        'requirements': 'PC-98 DOS, 16-bit real mode, 640x400 four-plane graphics, VSYNC IRQ, PIT',
        'expected': 'Decode eight original test-video frames and return to FreeCOM',
        'modifications': 'Unmodified executable; original test video; buzzer fallback, no PCM board',
    })
    manifests['fm98'].update({
        'version': 'Unmodified ADV98 fm86.c/h at 2f482577; original DOS entry harness',
        'requirements': 'PC-98 DOS, 188/18A OPN ports, VSYNC status polling',
        'expected': 'Two FM notes, no beeper; print INDEPENDENT FM ROUTINES OK and return to DOS',
        'build': 'tools/build_fm_test.py; ia16-elf-gcc 6.3.0 -O0 -march=v30 -mcmodel=tiny, ia16-elf-ld 2.39; no runtime libraries',
    })
    manifest = {
        'assets': {k: {'url': v[0], 'sha256': v[1]} for k, v in ASSETS.items()},
        'applications': manifests,
        'os_manifest': '../freedos/manifest.json',
        'firmware': 'original synthetic BIOS, guest/firmware.asm',
        'font': 'Shinonome 16, fixtures/fonts; see docs/dos-compatibility.md',
        'excluded': 'Release artwork, sprites, PLAY.M, SE.EFC, PMD and mouse TSR',
    }
    text = json.dumps(manifest, indent=2) + '\n'
    (DEST / 'manifest.json').write_text(text)
    expected = ROOT / 'fixtures/dos-apps-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif not expected.exists() or expected.read_text() != text:
        raise ValueError('DOS media differs from audited fixtures/dos-apps-manifest.json')
    (DEST / 'README.txt').write_text(
        'Independent DOS compatibility fixtures. See manifest.json and *-LICENSE.\n'
        'Redistribute with all source archives, tools/prepare_dos_apps.py,\n'
        'the complete fixtures/freedos output (GPL corresponding sources),\n'
        'and the repository MIT license for generated test art/video data.\n'
        'Original ADV98 release artwork/music and optional TSRs are excluded.\n'
        'Compatibility status and exact tests: docs/dos-compatibility.md.\n')
    print(json.dumps(manifests, indent=2))


if __name__ == '__main__':
    main()
