#!/usr/bin/env python3
"""Copy audited ADV98 scenes and build an HDD variant outside the checkout.

Explicit local use only: no downloads, publication changes or CI integration.
Run the existing FreeDOS, DOS-app and music fixture builders first.
"""
import argparse
import hashlib
import json
from pathlib import Path

from build_media import ROOT
from prepare_freedos import extract_fat_file
from prepare_hdd_fixture import image_with_files, load_boot


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    destination = args.destination.resolve()
    if destination == ROOT.resolve() or ROOT.resolve() in destination.parents:
        parser.error('Local performance media must stay outside the checkout')
    manifest = json.loads((ROOT / 'fixtures/music-manifest.json').read_text())
    scene = manifest['scene_workload']
    floppy = (ROOT / 'fixtures/dos-apps/music-scenes.hdm').read_bytes()
    if hashlib.sha256(floppy).hexdigest() != scene['image_sha256']:
        raise ValueError('music-scenes.hdm checksum mismatch')
    files = {name.encode(): extract_fat_file(floppy, name.encode()) for name in scene['files']}
    # Same guest files and song on the existing FAT12/IDE geometry. Only DOS's
    # boot-device configuration changes; retain all notices in both images.
    files[b'FDCONFIGSYS'] = b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n'
    boot, _ = load_boot()
    hdd, _ = image_with_files(boot, files)
    destination.mkdir(parents=True, exist_ok=True)
    if any((destination / name).is_symlink() for name in ['music-scenes.hdm', 'music-scenes.hdi']):
        raise ValueError('Local media outputs must not be symlinks')
    for name, data in [('music-scenes.hdm', floppy), ('music-scenes.hdi', hdd)]:
        (destination / name).write_bytes(data)
        print(name, hashlib.sha256(data).hexdigest())


if __name__ == '__main__':
    main()
