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
    # Keep the release flags and select pre-C23 C: this pinned JWasm typedefs bool.
    run(['make', '-s', '-f', 'GccUnix.mak', '-j2',
         'extra_c_flags=-DNDEBUG -O2 -std=gnu17'], compiler)
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
    # A second scenario keeps the same BGM running across repeated bitmap and
    # sprite reads. Preserve every byte/hash of the original music.hdm above.
    scenes = {**files, b'SCRIPT  TXT': (ROOT / 'guest/music-scenes.txt').read_bytes().replace(b'\n', b'\r\n'),
        b'NEXT    G98': files[b'CLASS   G98'][:13] + bytes(b ^ 255 for b in files[b'CLASS   G98'][13:])}
    scene_image = image_with_files(base[:1024], scenes)
    (DEST / 'music-scenes.hdm').write_bytes(scene_image)
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
        'os_and_game_provenance': ['freedos/manifest.json', 'dos-apps-manifest.json'],
        'scene_workload': {
            'image': 'dos-apps/music-scenes.hdm', 'image_sha256': sha(scene_image),
            'scenario': 'guest/music-scenes.txt',
            'scenario_sha256': sha((ROOT / 'guest/music-scenes.txt').read_bytes()),
            'files': {n.decode(): {'sha256': sha(v), 'rights': manifest_rights(n)} for n, v in scenes.items()},
            'changes': ['Original MIT looping scenario replaces SCRIPT.TXT',
                        'Original MIT NEXT.G98 inverts the generated CLASS.G98 planes'],
            'expected': 'Real FreeDOS compiles PLAY.MML, loads PMD, then unmodified ADV98 plays FM/SSG while Enter reloads backgrounds/sprites; no host music'},
        'rights_evidence': {
            'engine_and_palette': {'archive': 'adv98-engine-source.tar.gz',
                'palette_archive': 'adv98-scenario-source.tar.gz', 'notice': 'LICENSE',
                'notice_sha256': sha(source_file((DEST / 'adv98-engine-source.tar.gz').read_bytes(), 'LICENSE')),
                'applies_to': ['ADV98.EXE', 'ADV.PAL'], 'license': 'MIT; retain APP.LIC'},
            'original_assets': {'sources': ['guest/play.mml', 'guest/music-scenes.txt', 'tools/prepare_dos_apps.py', 'tools/prepare_music_fixture.py'],
                'applies_to': ['PLAY.MML', 'SCRIPT.TXT (scene workload)', 'CLASS.G98', 'NEXT.G98', 'C01_N.SPR', 'AUTOEXEC.BAT'],
                'license': 'Repository MIT; retain RUST.LIC; release art/music excluded'},
            'pmd_and_mc': {'archive': 'pmd-source.tar.gz', 'notice': 'readme.txt', 'notice_sha256': sha(permission),
                'applies_to': ['PMD.COM', 'MC.EXE'],
                'license': 'Author M.Kajihara grants free use of published sources, retains copyright; not MIT',
                'requirements': 'Retain complete source archive, adapted sources/build recipe and PMD.TXT with redistribution'},
            'dos': {'manifest': 'freedos/manifest.json', 'applies_to': ['boot sector', 'KERNEL.SYS', 'COMMAND.COM'],
                'license': 'GPL-2.0-or-later; retain COPYING and complete matching kernel/FreeCOM source archives'},
            'font': {'audit': 'fonts/README.md', 'notices': ['fonts/LICENSE', 'fonts/AUTHORS'],
                'license': 'Shinonome redistribution/conversion grant; preserve author and source-design notices; embedded in the core, not a NEC ROM'},
            'compiler': {'archive': 'jwasm-source.tar.gz', 'notice': 'License.txt',
                'notice_sha256': sha(source_file(blobs['jwasm-source.tar.gz'], 'License.txt')),
                'license': 'JWasm Sybase Open Watcom Public License; build tool only, not on guest disk'},
            'publication': 'Local and CI use/redistribution with the listed notices and complete corresponding sources. No music disks or new assets added to Pages.',
            'selection': 'ADV98 v0.1 is sufficient: actual interactive DOS game sample, FM plus SSG score and repeated graphical scene I/O. No second guest needed to reproduce measured scheduler starvation.',
            'alternative': {'url': 'https://delacannon.itch.io/advs98', 'version': 'v0.9.0 beta (page inspected 2026-11-06)',
                'status': 'Not acquired or redistributed; no claim of a full bundled-media audit',
                'reason': 'Author describes MIT engine but separate DOS/PMD/font/art terms and OPNA rhythm requiring sound ROM. ADV98 already reproduces starvation without those requirements.'}}}
    text = json.dumps(manifest, indent=2) + '\n'
    expected = ROOT / 'fixtures/music-manifest.json'
    if '--update-manifest' in sys.argv:
        expected.write_text(text)
    elif expected.read_text() != text:
        raise ValueError('music fixture differs from fixtures/music-manifest.json')
    (DEST / 'music-manifest.json').write_text(text)
    print('Music fixture verified:', sha(image))


def manifest_rights(name):
    if name in [b'MC      EXE', b'PMD     COM', b'PMD     TXT']:
        return 'PMD-permission.txt'
    if name in [b'KERNEL  SYS', b'COMMAND COM', b'COPYING    ']:
        return 'GPL-2.0-or-later'
    return 'MIT'


if __name__ == '__main__':
    build()
