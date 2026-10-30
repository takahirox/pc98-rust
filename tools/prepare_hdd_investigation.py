#!/usr/bin/env python3
"""Reproduce Issue #113 legal HDD alternatives; run after prepare_freedos.py.

Unmodified pinned GPL DOS/boot/FreeCOM, original MIT IPL/PROBE/POSTWAIT only.
Keep the entire fixtures/freedos directory with matching sources and notices.
No private image, additional driver, font or emulator implementation is used.
"""
import json
import struct
import sys

from build_media import ROOT, assemble
from prepare_freedos import extract_fat_file
from prepare_hdd_fixture import image_with_files, load_boot, sha


def verify_files(image, files, bits):
    """Walk on-disk chains separately from allocation; reject aliasing/corruption."""
    hidden = struct.unpack_from('<H', image, 4096 + 512 + 6)[0]
    heads, sectors = struct.unpack_from('<I', image, 24)[0], struct.unpack_from('<I', image, 20)[0]
    volume = image[4096 + hidden * heads * sectors * 512:]
    bps, spc, reserved, copies, roots, total, _, fatsecs = struct.unpack_from('<HBHBHHBH', volume, 11)
    assert bps == 512 and copies == 2
    root_start = reserved + copies * fatsecs
    data_start = root_start + (roots * 32 + 511) // 512
    fat = volume[reserved * 512:(reserved + fatsecs) * 512]
    assert fat == volume[(reserved + fatsecs) * 512:root_start * 512]
    root = volume[root_start * 512:data_start * 512]
    used, chains = set(), {}
    for name, expected in files.items():
        entry = next(root[i:i+32] for i in range(0, len(root), 32) if root[i:i+11] == name)
        cluster, length = struct.unpack_from('<HI', entry, 26)
        result, chain = bytearray(), []
        while cluster < ((1 << bits) - 8):
            assert 2 <= cluster < (total - data_start) // spc + 2
            assert cluster not in used
            used.add(cluster)
            chain.append(cluster)
            at = (data_start + (cluster - 2) * spc) * 512
            result += volume[at:at + spc * 512]
            if bits == 12:
                cluster = (int.from_bytes(fat[cluster * 3 // 2:cluster * 3 // 2 + 2], 'little') >> (4 if cluster & 1 else 0)) & 0xfff
            else:
                cluster = struct.unpack_from('<H', fat, cluster * 2)[0]
        assert length == len(expected) and result[:length] == expected
        chains[name.decode()] = chain
    return chains


def main():
    dest = ROOT / 'fixtures/freedos'
    base = (dest / 'minimal.hdm').read_bytes()
    assert sha(base) == json.loads((dest / 'manifest.json').read_text())['image_sha256']
    files = {name: extract_fat_file(base, name) for name in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']}
    files.update({
        b'FDCONFIGSYS': b'DOS=LOW\r\nSHELL=A:\\COMMAND.COM A:\\ /P /E:256\r\n',
        b'PROBE   COM': assemble('guest/probe.asm'),
        b'PROBE   ASM': (ROOT / 'guest/probe.asm').read_bytes(),
        b'POSTWAITCOM': assemble('guest/postwait.asm'),
        b'POSTWAITASM': (ROOT / 'guest/postwait.asm').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
    })
    results = {}
    for name, bits, fragmented, command in [
        ('fragmented12', 12, True, ''),
        ('fragmented16', 16, True, ''),
        ('wait', 12, False, 'POSTWAIT\r\n'),
        ('maskwait', 12, False, 'POSTWAIT M\r\n'),
    ]:
        selected = {**files, b'AUTOEXECBAT': ('@echo off\r\nPROMPT $p$g\r\n' + command + 'echo HDD SHELL READY\r\n').encode()}
        boot, source = load_boot(bits)
        image, _ = image_with_files(boot, selected, fat_bits=bits, fragmented=fragmented)
        chains = verify_files(image, selected, bits)
        if fragmented:
            assert chains['KERNEL  SYS'][1] != chains['KERNEL  SYS'][0] + 1
        (dest / (name + '.hdi')).write_bytes(image)
        (dest / f'hdd-boot-fat{bits}.asm').write_bytes(source)
        results[name] = {
            'sha256': sha(image), 'fat_bits': bits, 'fragmented': fragmented,
            'geometry': list(struct.unpack_from('<III', image, 20))[::-1],
            'partition_cylinder': 1 if bits == 12 else 2,
            'kernel_chain': chains['KERNEL  SYS'],
            'boot_source_sha256': sha(source),
            'files': {k.decode(): {'sha256': sha(v), 'bytes': len(v)} for k, v in selected.items()},
        }
        print(name, sha(image))
    (dest / 'hdd-kernel.sys').write_bytes(files[b'KERNEL  SYS'])
    manifest = {
        'license': 'GPL-2.0-or-later (unmodified boot/kernel/FreeCOM); MIT (original IPL/PROBE/POSTWAIT/configuration)',
        'redistribution': 'Retain entire fixtures/freedos with complete matching kernel/FreeCOM sources and notices; excluded from web/',
        'sources': {name: sha((ROOT / name).read_bytes()) for name in ['guest/hdd-ipl.asm', 'guest/probe.asm', 'guest/postwait.asm']},
        'slot_budget': 8_000_000, 'images': results,
    }
    text = json.dumps(manifest, indent=2) + '\n'
    pin = ROOT / 'fixtures/hdd-investigation-manifest.json'
    if '--update-manifest' in sys.argv:
        pin.write_text(text)
    elif pin.read_text() != text:
        raise ValueError('HDD investigation differs from pinned manifest')


if __name__ == '__main__':
    main()
