#!/usr/bin/env python3
"""Build a published third-party guest music driver/compiler, then a DOS disk.

JWasm is a build tool, PMD and MC are guest DOS programs; none is emulation code.
MC compiles the original score INSIDE FreeDOS at boot. ADV98 starts playback.
"""
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

from prepare_dos_apps import DEST, ROOT, fetch, sha, source_file
from prepare_freedos import extract_fat_file, image_with_files

ASSETS = {
    'pmd-source.tar.gz': (
        'https://codeload.github.com/d2lmirrors/pmd/tar.gz/c620dc95c5e47970e7839cb5f0b7b9ab742d4f46',
        'b25c84ac8cbaf5c4f9ae4ad870b6f08749ff7177a2bd6833e4c10adc199e9cc3'),
    'jwasm-source.tar.gz': (
        'https://codeload.github.com/JWasm/JWasm/tar.gz/a5c4ea03cc0545a15d81a354251b5f534bef7a1b',
        'dcdd11e965ce065123a4a061dd68b729b6e6b551f1157eb3130af3f28659bc1a'),
}


def run(args, cwd):
    result = subprocess.run(args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(result.stdout.decode(errors='replace')[-3000:])


def build():
    blobs = {n: fetch(n, *a) for n, a in ASSETS.items()}
    build_dir = DEST / 'music-build'
    build_dir.mkdir(exist_ok=True)
    # Safe extraction applies only to a compiler source archive. Guest files
    # below are read by exact member name, with their original archive retained.
    with tarfile.open(fileobj=io.BytesIO(blobs['jwasm-source.tar.gz'])) as tar:
        if sum(m.size for m in tar) > 32 * 1024 * 1024:
            raise ValueError('oversize compiler source')
        tar.extractall(build_dir, filter='data')
    compiler = build_dir / 'JWasm-a5c4ea03cc0545a15d81a354251b5f534bef7a1b'
    run(['make', '-s', '-f', 'GccUnix.mak', '-j2'], compiler)
    assembler = compiler / 'GccUnixR/jwasm'
    provenance = {}
    for directory, source_name, output, mode in [
        ('pmd', 'pmd.asm', 'PMD.COM', '-bin'), ('mc', 'mc.asm', 'MC.EXE', '-mz')]:
        target = build_dir / directory
        target.mkdir(exist_ok=True)
        with tarfile.open(fileobj=io.BytesIO(blobs['pmd-source.tar.gz'])) as tar:
            for member in tar:
                parts = member.name.split('/')
                if len(parts) == 3 and parts[1] == directory and member.isfile():
                    (target / parts[2].lower()).write_bytes(tar.extractfile(member).read())
        path = target / source_name
        original = path.read_bytes()
        adapted = original.replace(b'ver\t\tequ\t"4.8s"', b'ver\t\ttextequ\t<"4.8s">')
        if directory == 'pmd':
            adapted = adapted.replace(b'resmes\tequ\t"PMD ver.",ver', b'resmes\ttextequ\t<"PMD ver.",ver>')
            adapted = adapted.replace(b'\tloop\tdin0',
                b'\tloop\tshort din0_bridge\n\tjmp\tshort din0_done\ndin0_bridge:\n\tjmp\tdin0\ndin0_done:')
        path.write_bytes(adapted)
        run([str(assembler), mode, '-Zm', '-Fo' + output, source_name], target)
        binary = (target / output).read_bytes()
        (DEST / output).write_bytes(binary)
        provenance[output] = {'sha256': sha(binary), 'source': directory + '/' + source_name.upper(),
            'source_sha256': sha(original), 'adapted_sha256': sha(adapted),
            'rights': 'M.Kajihara: freely use published sources; copyright retained; see PMD-permission.txt',
            'build': 'JWasm ' + mode + ' -Zm -Fo' + output + ' ' + source_name}
    permission = source_file(blobs['pmd-source.tar.gz'], 'readme.txt')
    (DEST / 'PMD-permission.txt').write_bytes(permission)
    base = (DEST / 'adv98.hdm').read_bytes()
    names = [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ', b'FDCONFIGSYS', b'ADV98   EXE',
             b'SCRIPT  TXT', b'ADV     PAL', b'CLASS   G98', b'C01_N   SPR', b'APP     LIC']
    files = {n: extract_fat_file(base, n) for n in names}
    files.update({b'MC      EXE': (DEST / 'MC.EXE').read_bytes(),
        b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
        b'PMD     COM': (DEST / 'PMD.COM').read_bytes(), b'PMD     TXT': permission,
        b'PLAY    MML': (ROOT / 'guest/play.mml').read_bytes().replace(b'\n', b'\r\n'),
        b'AUTOEXECBAT': b'@echo off\r\nMC /V PLAY.MML\r\nPMD\r\nADV98\r\n'})
    image = image_with_files(base[:1024], files)
    (DEST / 'music.hdm').write_bytes(image)
    manifest = {'image_sha256': sha(image),
        'assets': {n: {'url': a[0], 'sha256': a[1]} for n, a in ASSETS.items()},
        'programs': provenance,
        'score': {'source': 'guest/play.mml', 'sha256': sha((ROOT / 'guest/play.mml').read_bytes()), 'license': 'MIT'},
        'modifications': ['Case-normalized include filenames for Unix',
            'EQU string macros changed to TEXTEQU for JWasm',
            'One out-of-range LOOP uses a flags-preserving branch trampoline'],
        'files': {n.decode(): {'sha256': sha(v), 'rights':
            'PMD-permission.txt' if n in [b'MC      EXE', b'PMD     COM', b'PMD     TXT']
            else 'GPL-2.0-or-later' if n in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']
            else 'MIT'} for n, v in files.items()},
        'requirements': ['Guest FreeDOS/FreeCOM', 'YM2203 FM/SSG', 'SSG port A interrupt-selection bits',
                         'FM timers and IRQ12', 'DOS TSR', 'Unmodified ADV98 #bgm PLAY.M'],
        'redistribution': 'Retain complete fixtures/freedos and dos-apps outputs, including PMD/MC sources, permission, compiler notices and original MIT score; outside Pages.',
        'os_and_game_provenance': ['freedos/manifest.json', 'dos-apps-manifest.json']}
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/music-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('music fixture differs from fixtures/music-manifest.json')
    (DEST / 'music-manifest.json').write_text(text)
    print('Music fixture verified:', sha(image))


if __name__ == '__main__':
    build()
