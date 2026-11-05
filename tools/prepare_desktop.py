#!/usr/bin/env python3
"""Publish only a cleared 128 MiB FAT16 FreeDOS desktop prefix + matching sources.

The prefix ends on a 64 KiB import page; the remaining logical bytes are zero.
No raw HDD is written, allocated, checked into Git or placed in the site tree.
"""
import io
import json
import shutil
import struct
import sys
import tarfile

from build_media import ROOT, assemble
from prepare_freedos import ASSETS, DEST, extract_fat_file, main as prepare
from prepare_hdd_fixture import load_boot, sha

PUBLIC = ROOT / 'web/freedos'
PIN = ROOT / 'fixtures/desktop-manifest.json'
HEADER, CAPACITY, PAGE = 4096, 128 * 1024 * 1024, 65536


def build():
    # Revalidate cached archives and regenerate the minimal input before use.
    prepare()
    base = (DEST / 'minimal.hdm').read_bytes()
    files = {n: extract_fat_file(base, n) for n in
             [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']}
    files.update({
        b'FDCONFIGSYS': b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n',
        b'AUTOEXECBAT': b'@echo off\r\nPROMPT $p$g\r\necho FreeDOS desktop ready\r\n',
        b'PROBE   COM': assemble('guest/probe.asm'),
        b'PROBE   ASM': (ROOT / 'guest/probe.asm').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
    })
    cylinders, heads, sectors = 512, 16, 32
    hidden, reserved, fatsecs, roots, spc = 512, 2, 128, 512, 8
    total = CAPACITY // 512 - hidden
    rootsecs = roots * 32 // 512
    datastart = reserved + 2 * fatsecs + rootsecs
    clusters = (total - datastart) // spc
    assert 4085 <= clusters < 65525 and (clusters + 2) * 2 <= fatsecs * 512
    boot, source = load_boot(16)
    boot = bytearray(boot)
    struct.pack_into('<HBHBHHBHHHII', boot, 11, 512, spc, reserved, 2, roots,
                     0, 0xf8, fatsecs, sectors, heads, hidden, total)
    boot[36:62] = b'\x80\x00\x29' + struct.pack('<I', 0x98524454) + b'FREEDOS HDD' + b'FAT16   '
    struct.pack_into('<IHHBIHIIHH', boot, 62, hidden, datastart, 512, 0,
                     reserved + 2 * fatsecs, rootsecs, reserved, datastart, 0, 0x60)
    fat, root = bytearray(fatsecs * 512), bytearray(roots * 32)
    fat[:4] = b'\xf8\xff\xff\xff'
    writes, cluster = [], 2
    for i, (name, data) in enumerate(files.items()):
        count = (len(data) + spc * 512 - 1) // (spc * 512)
        root[i*32:i*32+11] = name
        root[i*32+11] = 0x20
        struct.pack_into('<HI', root, i*32+26, cluster, len(data))
        for c in range(cluster, cluster + count):
            struct.pack_into('<H', fat, c*2, c+1 if c+1 < cluster+count else 0xffff)
        writes.append((HEADER + (hidden+datastart+(cluster-2)*spc)*512, data))
        cluster += count
    header, part = bytearray(HEADER), bytearray(512)
    struct.pack_into('<IIIIII', header, 8, HEADER, CAPACITY, 512, sectors, heads, cylinders)
    part[:2] = b'\xa1\x81'
    struct.pack_into('<BBH', part, 4, 0, 0, 1)
    struct.pack_into('<BBH', part, 8, 0, 0, 1)
    struct.pack_into('<BBH', part, 12, sectors-1, heads-1, cylinders-1)
    part[16:32] = b'FreeDOS FAT16   '
    ipl = assemble('guest/hdd-ipl.asm')
    writes += [(0, header), (HEADER, ipl), (HEADER+512, part),
               (HEADER+hidden*512, boot), (HEADER+(hidden+reserved)*512, fat),
               (HEADER+(hidden+reserved+fatsecs)*512, fat),
               (HEADER+(hidden+reserved+2*fatsecs)*512, root)]
    end = max(at+len(data) for at, data in writes)
    prefix = bytearray((end+PAGE-1)//PAGE*PAGE)
    for at, data in writes:
        prefix[at:at+len(data)] = data
    # Independent directory/FAT walk, not just a checksum of the builder input.
    assert prefix[HEADER+hidden*512+510:HEADER+hidden*512+512] == b'\x55\xaa'
    assert struct.unpack_from('<IIIIII', prefix, 8) == (HEADER, CAPACITY, 512, 32, 16, 512)
    for i, (name, data) in enumerate(files.items()):
        at = HEADER+(hidden+reserved+2*fatsecs)*512+i*32
        assert prefix[at:at+11] == name
        c, size = struct.unpack_from('<HI', prefix, at+26)
        result, seen = bytearray(), set()
        while c < 0xfff8:
            assert 2 <= c < clusters+2 and c not in seen
            seen.add(c)
            start = HEADER+(hidden+datastart+(c-2)*spc)*512
            result += prefix[start:start+spc*512]
            c = struct.unpack_from('<H', prefix, HEADER+(hidden+reserved)*512+c*2)[0]
        assert bytes(result[:size]) == data
    # Hash the logical image using bounded zero chunks, never a full raw copy.
    import hashlib
    digest = hashlib.sha256(prefix)
    remaining = HEADER+CAPACITY-len(prefix)
    while remaining:
        n = min(remaining, PAGE)
        digest.update(bytes(n))
        remaining -= n
    manifest = {
        'bytes': HEADER+CAPACITY, 'prefix_bytes': len(prefix), 'prefix_sha256': sha(prefix),
        'image_sha256': digest.hexdigest(), 'geometry': [512, 16, 32, 512],
        'partition_lba': hidden, 'partition_sectors': total,
        'fat': {'bits': 16, 'clusters': clusters, 'sectors_per_cluster': spc,
                'sectors_per_fat': fatsecs, 'root_lba': hidden+reserved+2*fatsecs,
                'data_lba': hidden+datastart, 'free_clusters': clusters-(cluster-2)},
        'boot_source_sha256': sha(source), 'boot_build': 'nasm -f bin -DNEC98HDD -DISFAT16 boot.asm',
        'ipl_sha256': sha(ipl),
        'inputs': {k: {'url': v[0], 'sha256': v[1]} for k, v in ASSETS.items()},
        'files': {n.decode(): {'bytes': len(d), 'sha256': sha(d)} for n, d in files.items()},
        'license': 'GPL-2.0-or-later (kernel/FreeCOM/boot); MIT (IPL/PROBE/configuration)',
    }
    text = json.dumps(manifest, indent=2)+'\n'
    if '--update-manifest' in sys.argv:
        PIN.write_text(text)
    elif PIN.read_text() != text:
        raise ValueError('Desktop differs from fixtures/desktop-manifest.json')
    PUBLIC.mkdir(parents=True, exist_ok=True)
    (PUBLIC / 'desktop.bin').write_bytes(prefix)
    (PUBLIC / 'manifest.json').write_text(text)
    # GPLv2 section 3(a): same-site, complete matching source archives with
    # original notices/build scripts, plus all original disk assembly helpers.
    for name in ['kernel-source.tar.gz', 'freecom-source.tar.gz', 'COPYING',
                 'FreeCOM-LICENSE', 'distribution-READMEja.htm']:
        shutil.copyfile(DEST/name, PUBLIC/name)
    helpers = ['guest/timer.asm', 'guest/keymap.asm', 'guest/hblank.asm',
               'guest/lioinit.asm', 'guest/liotest.asm', 'guest/drawmode.asm',
               'guest/drawtest.asm', 'guest/hdd-ipl.asm', 'guest/probe.asm', 'LICENSE',
               'tools/prepare_desktop.py', 'tools/prepare_freedos.py',
               'tools/prepare_hdd_fixture.py', 'tools/build_media.py',
               'fixtures/desktop-manifest.json']
    with tarfile.open(PUBLIC/'desktop-source.tar', 'w', format=tarfile.USTAR_FORMAT) as tar:
        for name in helpers:
            data = (ROOT/name).read_bytes()
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode, entry.mtime = len(data), 0o644, 0
            tar.addfile(entry, io.BytesIO(data))
        for name in [b'FDCONFIGSYS', b'AUTOEXECBAT']:
            data = files[name]
            entry = tarfile.TarInfo(name.decode())
            entry.size, entry.mode, entry.mtime = len(data), 0o644, 0
            tar.addfile(entry, io.BytesIO(data))
    (PUBLIC/'NOTICE.txt').write_text(
        'FreeDOS(98) desktop, GPL version 2 or later; no warranty.\n'
        'Kernel: DOS-C Copyright 1995, 1996 Pasquale J. Villani; portions\n'
        'copyright other FreeDOS contributors, including the PC-98/DBCS authors.\n'
        'FreeCOM: Copyright FreeCOM contributors; includes the SUPPL library.\n'
        'All original per-file copyright/license notices and build instructions\n'
        'are preserved in the complete kernel-source.tar.gz and freecom-source.tar.gz.\n'
        'See COPYING and FreeCOM-LICENSE. Exact commits, URLs, binary/source hashes,\n'
        'geometry and selected guest files: manifest.json.\n'
        'Kernel and FreeCOM binaries are unmodified. GPL HDD boot code is assembled\n'
        'unchanged; its BPB/layout fields and filesystem are configured for FAT16.\n'
        'Original MIT IPL/PROBE/configuration and reproducible assembly scripts:\n'
        'desktop-source.tar (includes LICENSE). No other DOS utilities are installed.\n'
        'To redistribute the desktop, accompany it with these complete matching\n'
        'sources and notices, under GPLv2 section 3(a); upstream links alone are\n'
        'not the source distribution. Source downloads have equal access here.\n'
        'The Rust emulator remains MIT; these programs execute only as guests.\n')
    publication = {p.name: sha(p.read_bytes()) for p in sorted(PUBLIC.iterdir()) if p.is_file()}
    publication_pin = ROOT / 'fixtures/desktop-publication.json'
    publication_text = json.dumps(publication, indent=2)+'\n'
    if '--update-manifest' in sys.argv:
        publication_pin.write_text(publication_text)
    elif publication_pin.read_text() != publication_text:
        raise ValueError('Desktop publication/source artifact checksum mismatch')
    print(f'Desktop verified: {len(prefix)} network bytes; {HEADER+CAPACITY} logical bytes')


if __name__ == '__main__':
    build()
