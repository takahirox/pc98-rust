#!/usr/bin/env python3
"""Build the independent ADV98 FM routines with our minimal DOS harness.

Requires ia16-elf-gcc 6.3.0 (TK Chia PPA); no libc or other guest dependency.
The copied MIT fm86.c/h are unmodified upstream files; never emulator code.
"""
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'fixtures/fm98/fm98.com'
with tempfile.TemporaryDirectory() as directory:
    binary = Path(directory) / 'fm98.com'
    objects = []
    for index, source in enumerate(['guest/fm98/start.S', 'fixtures/fm98/fm86.c']):
        obj = Path(directory) / f'{index}.o'
        # At -Os the upstream hardcoded AL asm and generic byte "a" constraint
        # can select AH in this compiler. -O0 uses AL without changing the guest.
        subprocess.run([
            'ia16-elf-gcc', '-O0', '-march=v30', '-mcmodel=tiny',
            '-ffreestanding', '-fno-builtin', '-c', source, '-o', str(obj),
        ], cwd=ROOT, check=True)
        objects.append(str(obj))
    # Invoke ld directly, avoiding GCC's implicit DOS runtime linker script.
    subprocess.run(['ia16-elf-ld', '-T', 'guest/fm98/dos.ld', *objects,
                    '-o', str(binary)], cwd=ROOT, check=True)
    data = binary.read_bytes()
    if '--check' in sys.argv:
        assert data == OUTPUT.read_bytes(), 'stale FM test; run tools/build_fm_test.py'
    else:
        OUTPUT.write_bytes(data)
    print(f'Independent FM guest: {len(data)} bytes')
