#!/usr/bin/env python3
"""Build a sparse 512 MiB legitimate FAT16 FreeDOS(98) test disk.

Unmodified checksum-pinned GPL kernel/FreeCOM/HDD boot source, original MIT IPL,
configuration and blank filler file. Retain fixtures/freedos sources/notices.
No acquired emulator implementation or private media. Never publish this image.
"""
import json
import struct
import sys
from build_media import ROOT, assemble
from prepare_freedos import extract_fat_file
from prepare_hdd_fixture import load_boot, sha


def main():
    dest = ROOT / 'fixtures/freedos'
    base = (dest / 'minimal.hdm').read_bytes()
    assert sha(base) == json.loads((dest / 'manifest.json').read_text())['image_sha256']
    files = {name: extract_fat_file(base, name) for name in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']}
    files.update({
        b'FDCONFIGSYS': b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n',
        b'AUTOEXECBAT': b'@echo off\r\nPROMPT $p$g\r\ntype HIGH.TXT\r\nif exist NEW.TXT type NEW.TXT\r\necho LARGE DOS WRITE>NEW.TXT\r\ntype NEW.TXT\r\necho LARGE DOS READY\r\n',
        b'PROBE   COM': assemble('guest/probe.asm'),
        b'PROBE   ASM': (ROOT / 'guest/probe.asm').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
    })
    cylinders, heads, sectors = 2048, 16, 32
    hidden, reserved, fatsecs, roots, spc = 512, 2, 128, 512, 32
    total = cylinders * heads * sectors - hidden
    rootsecs = roots * 32 // 512
    datastart = reserved + fatsecs * 2 + rootsecs
    clusters = (total - datastart) // spc
    assert 4085 <= clusters < 65525 and (clusters+2)*2 <= fatsecs*512
    boot, source = load_boot(16)
    boot = bytearray(boot)
    struct.pack_into('<HBHBHHBHHHII', boot, 11, 512, spc, reserved, 2, roots,
                     0, 0xf8, fatsecs, sectors, heads, hidden, total)
    boot[36:62] = b'\x80\x00\x29' + struct.pack('<I', 0x98524c47) + b'LARGE DOS  ' + b'FAT16   '
    struct.pack_into('<IHHBIHIIHH', boot, 62, hidden, datastart, 512, 0,
                     reserved + fatsecs * 2, rootsecs, reserved, datastart, 0, 0x60)
    fat = bytearray(fatsecs*512)
    fat[:4] = b'\xf8\xff\xff\xff'
    root = bytearray(roots*32)
    writes = []
    next_cluster = 2
    def entry(name, data, first, count, index):
        at = index*32
        root[at:at+11] = name
        root[at+11] = 0x20
        struct.pack_into('<HI', root, at+26, first, len(data) if data is not None else count*spc*512)
        for c in range(first,first+count):
            struct.pack_into('<H', fat, c*2, c+1 if c+1<first+count else 0xffff)
        if data is not None:
            writes.append(((hidden+datastart+(first-2)*spc)*512, data))
    for index,(name,data) in enumerate(files.items()):
        count = (len(data)+spc*512-1)//(spc*512)
        entry(name,data,next_cluster,count,index)
        next_cluster += count
    # A real zero-filled file occupies low clusters, so DOS creates NEW.TXT
    # beyond 128 MiB without any guest-specific emulator behavior or hook.
    entry(b'LOW     BIN',None,next_cluster,8192-next_cluster,len(files))
    high = b'HIGH FAT16 SOURCE\r\n'
    entry(b'HIGH    TXT',high,8192,1,len(files)+1)
    header=bytearray(4096)
    struct.pack_into('<IIIIII',header,8,4096,cylinders*heads*sectors*512,512,sectors,heads,cylinders)
    ipl=assemble('guest/hdd-ipl.asm')
    part=bytearray(512)
    part[:2]=b'\xa1\x81'
    struct.pack_into('<BBH',part,4,0,0,1)
    struct.pack_into('<BBH',part,8,0,0,1)
    struct.pack_into('<BBH',part,12,31,15,2047)
    part[16:32]=b'FreeDOS FAT16   '
    writes += [(0,ipl),(512,part),(hidden*512,boot),
               ((hidden+reserved)*512,fat),((hidden+reserved+fatsecs)*512,fat),
               ((hidden+reserved+2*fatsecs)*512,root)]
    # Hash the small data plane and pin geometry/positions, not a second giant
    # materialized byte array. Browser acceptance checks the entire export.
    manifest={
        'image':'freedos/large-fat16.hdi', 'bytes':4096+cylinders*heads*sectors*512,
        'geometry':[cylinders,heads,sectors,512], 'partition_lba':hidden,'partition_sectors':total,
        'fat':{'bits':16,'clusters':clusters,'sectors_per_cluster':spc,'sectors_per_fat':fatsecs,'data_lba':hidden+datastart},
        'high_file_lba':hidden+datastart+(8192-2)*spc,
        'first_free_lba':hidden+datastart+(8193-2)*spc,
        'boot_source_sha256':sha(source), 'ipl_sha256':sha(ipl),
        'files':{name.decode():sha(data) for name,data in files.items()},
        'extents': [{'offset':at,'length':len(data),'sha256':sha(data)} for at,data in sorted(writes)],
        'license':'GPL-2.0-or-later (unmodified boot/kernel/FreeCOM); MIT (IPL/PROBE/configuration); generated zero filler',
        'sources':'Keep complete fixtures/freedos, including matching source archives/notices; test only; excluded from web',
    }
    text=json.dumps(manifest,indent=2)+'\n'
    pin=ROOT/'fixtures/large-dos-manifest.json'
    if '--update-manifest' in sys.argv:
        pin.write_text(text)
    elif pin.read_text()!=text:
        raise ValueError('large DOS fixture differs from manifest')
    with (dest/'large-fat16.hdi').open('wb') as image:
        image.truncate(manifest['bytes'])
        image.write(header)
        for at,data in writes:
            image.seek(4096+at)
            image.write(data)
    (dest/'hdd-boot-fat16.asm').write_bytes(source)
    print('Large FAT16 DOS fixture verified; first free LBA:',manifest['first_free_lba'])


if __name__=='__main__':
    main()
