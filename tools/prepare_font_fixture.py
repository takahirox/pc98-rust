#!/usr/bin/env python3
"""Build original MIT font controls on the existing pinned GPL FreeDOS fixture.

No PIPOO files are needed. Keep the full fixtures/freedos folder and its matching
source archives/notices together; these disks are never added to web/.
"""
from pathlib import Path
import subprocess
import tempfile
from build_media import ROOT, assemble
from prepare_freedos import DEST, extract_fat_file, image_with_files

# Preserve the exact original Issue #16 control, including its trailing scratch.
# The databook specifies leading scratch; userfont.asm tests that layout.
CONTROL = '''; Original MIT investigation probe for Issue #13.
bits 16
org 100h
push cs
pop bx
mov cx,pattern
mov dx,7621h
mov ah,1ah
int 18h
mov ax,4c00h
int 21h
pattern:
times 16 dw 08181h
dw 0 ; BIOS scratch workspace
'''


def main():
    base = (DEST / 'minimal.hdm').read_bytes()
    common = {n: extract_fat_file(base, n) for n in [
        b'KERNEL  SYS', b'COMMAND COM', b'FDCONFIGSYS', b'COPYING    ']}
    with tempfile.TemporaryDirectory(prefix='pc98-fontload-') as directory:
        source = Path(directory) / 'fontload.asm'
        source.write_text(CONTROL)
        binary = source.with_suffix('.com')
        subprocess.run(['nasm', '-f', 'bin', '-o', str(binary), str(source)], check=True)
        programs = {'fontload': binary.read_bytes(), 'userfont': assemble('guest/userfont.asm')}
    for name, program in programs.items():
        files = {**common,
                 (name.upper().ljust(8) + 'COM').encode(): program,
                 b'RUST    LIC': (ROOT / 'LICENSE').read_bytes(),
                 b'AUTOEXECBAT': (f'@echo off\r\n{name}\r\necho {name.upper()} RETURNED\r\n').encode()}
        (DEST / (name + '.hdm')).write_bytes(image_with_files(base[:1024], files))
        print(f'Built original {name} control on pinned FreeDOS')


if __name__ == '__main__':
    main()
