# Independent FM guest routines

`fm86.c` and `fm86.h` are unmodified from
[ADV98](https://github.com/yarufu/pc98/tree/2f482577433f89abd3166e95209f3ffea21b9bbc/src),
commit `2f482577433f89abd3166e95209f3ffea21b9bbc`. They are guest hardware-driving
code, not emulator/chip implementations. Their original MIT notice is LICENSE.

The original MIT DOS harness is `guest/fm98/start.S`, linked with
`guest/fm98/dos.ld`. It calls `se86_init`, `se86_play_beep` and
`se86_play_beep2`, prints a DOS completion marker and exits through actual INT 21h.
It imports no C runtime or driver. It is a test of independent routines with a
new harness, not a third independent application or a claim that ADV98's released
game produces sound without PMD.

To reproduce the 795-byte COM, install the author's
[ia16 toolchain](https://launchpad.net/~tkchia/+archive/ubuntu/build-ia16) on Linux
(`gcc-ia16-elf`, `binutils-ia16-elf`), then run:

```sh
python3 tools/build_fm_test.py --check
```

Verified compiler: GCC 6.3.0 package
`6.3.0-20260612.00-ppa260612104~noble`; binutils 2.39 package
`2.39-20260423.22-ppa260423231~noble`. The build uses `-O0` because the source's
hardcoded AL assembly and generic byte `a` constraints select AH under `-Os` in
this compiler. No source changes were needed. The generated COM SHA-256 is
`0ff73bd58e4293137738e66b1bb67e324f24327ff370581243f1815f5beb28ea`.
The fixture preparation script checks this binary hash and compares both copied
source files byte-for-byte with the checksum-pinned full upstream source archive.
