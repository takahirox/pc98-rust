# File and guest-media license audit

Emulator code and ordinary build/test dependencies are separate from guest
software. The emulator is written from scratch; no existing emulation code is
compiled, embedded, translated or forked. Its Rust dependency tree is empty.
The only npm dependency is Playwright and its test tooling, not a runtime core.

## Original files

`src/`, `guest/firmware.asm`, `guest/game.asm`, `guest/probe.asm`, `tools/`, tests,
the web host, documentation and the original replacement glyph shapes use the
repository [MIT license](../LICENSE), copyright 2026 pc98-rust contributors.
`guest/firmware.bin` and `web/media/startrail.hdm` are generated solely from those
assembly sources and zero padding. There are no external game assets. The font is
new Latin-only glyph data in `src/font.rs`; it is not copied from NEC or another
font package. `tools/build_media.py --check` reproduces these binaries offline
using NASM and detects stale checked-in artifacts.

## HelloWorld-PC98

Upstream: [TakedaHiromasa/HelloWorld-PC98](https://github.com/TakedaHiromasa/HelloWorld-PC98),
commit **4f1ea8e8d3196d4f2b53b17d9ac08e38d43df247**. Only these artifacts are copied:

| Local file | Origin and treatment |
| --- | --- |
| `fixtures/hello98/hello98.nas` | Unmodified `src/hello98.nas`, independently authored assembly |
| `fixtures/hello98/LICENSE` | Unmodified upstream MIT license, including its original `Copyright (c) 2018` notice |
| `fixtures/hello98/boot.bin` | Bytes 0–1023 of upstream `src/hello98.TFD`; reviewed against the source's boot/header, text-print loop, PIT/beep setup, HLT loop and message |
| `web/media/hello98.tfd` | That exact boot sector plus zero padding to 1,261,568 bytes; all other filesystem/tooling/slack data excluded |
| `web/hello98-LICENSE.txt` | Copy of the same MIT notice in the static distribution |

The original full TFD SHA-256 is
`81db6564aed59fb44085253b2488d31fe622a883d22d31d363b3de0ae0823f29`.
The imported first 1024 bytes hash is
`bdf20204b963227b826623f2a16a43f9d9cf8e0d4f1fef337b9fbd2276606cda`.
The cleared rebuilt image hash is
`8003c97b0c028a379e2572bf0f2cf86a32d0a624d408c7273de756f37413d406`.
The reproducible image construction preserves the independently authored machine
code byte-for-byte, including the immediate OUT port truncation. We do not copy
upstream `hood`, assembler binaries, other tools, or other disk filesystem data.
The native and browser tests boot this disk and assert real text pixels and beep.
This is bare-metal floppy software, **not a firmware ROM**.

## FreeDOS(98): downloaded test guest, GPLv2 or later

`tools/prepare_freedos.py` fetches pinned public guest releases and matching source
archives, checking SHA-256 before using any download. Outputs live in the ignored
`fixtures/freedos/` directory; no GPL DOS binaries are included in the static
`web/` distribution or linked into the Rust core.

The [WebNP2 packaging notes](https://github.com/uraraworks/WebNP2/blob/master/README.md#bundled-freedos98-boot-floppy)
identified the original guest release. We verified its `fd98_2hd.img` is identical
to WebNP2's renamed XDF (1,261,568 bytes). We fetch the binary directly from the
original release and never download WebNP2/NP2 emulation code.

- Boot sector and 8086-compatible kernel: [lpproj/fdkernel](https://github.com/lpproj/fdkernel/releases/tag/test-20220120-cherrypick),
  tag `test-20220120-cherrypick`, source commit
  `679eda7eeeac6cc68bad4d3ff9b055e672529731`, kernel build 2043 DBCS dated 2022-01-20.
  `fd98_2hd144_20220123.zip` SHA-256:
  `ad12a889bae25dde5be566fccc44188f21762e241b7ff14ced3d2eac5114f5fa`.
- Unpacked PC-98 shell: [lpproj/freecom_dbcs2](https://github.com/lpproj/freecom_dbcs2/releases/tag/dbcswip-20220117),
  tag `dbcswip-20220117`, source commit
  `86a1a89eadbc14d8ff3f52e62dc51e98210de216`. The selected artifact is specifically
  `nec98/japanese/command.com` from `freecom_dbcswip-20220117-bin.zip`, not an IBM
  build or another sample asset. Archive SHA-256:
  `f219de9fdb3f543e8e71f216d37d4aa231f77a04e95c7d804dc976ba9a994a0e`.

The script rebuilds FAT12 on a zeroed image, copying only boot code, `KERNEL.SYS`,
`COMMAND.COM`, GPL `COPYING`, the original MIT `PROBE.COM`, and minimal original
configuration/AUTOEXEC files. No unrelated utilities, country data, XMS drivers,
filesystem slack or proprietary files from the full guest distribution are copied.
The full distribution is a downloaded input only, not included in the derived
image. The real-mode profile excludes its 286 XMS driver; it does not fake A20 or
protected mode to install that driver.

The generated `manifest.json` records hashes for every selected file, the final
image, input archives, source archives and exact source URLs. Expected minimal
image SHA-256:
`015faaa0dfb4c23f7a8aa4b74bbad2d460afc55c64517c5b4ea7330d4a8f8d1e`.
This fixture has been booted through the synthetic IPL/FDC into the actual FreeDOS
kernel and FreeCOM, then used to execute PROBE.COM through DOS.

The script writes the full matching `kernel-source.tar.gz` and
`freecom-source.tar.gz`, original distribution documentation, `COPYING`, the
FreeCOM license, provenance manifest and a redistribution README **beside** the
image. These source archives include upstream source and build scripts. No changes
to boot, kernel or shell are made; the filesystem/configuration is rebuilt by the
script. Anyone redistributing the derived image must preserve its notices and
accompany it with this complete source/license output, rather than relying on a
repository badge or a vague offer of source. The preparation script and original
PROBE source must accompany it as well. Nothing requires bundling or accepting
proprietary media.

## Playable-demo selection

[ADV98](https://github.com/yarufu/pc98/blob/main/README.md) is a DOS-based 16-color
engine, tested on PC-9821 systems with optional sound boards. It is not an 8-color
VM acceptance candidate. [Space Panicco's specific rights notice](https://github.com/gikonekos/Space-Panicco-Archive/blob/main/RIGHTS.md)
does not provide a clear unrestricted redistribution basis for its original
binaries, so none are copied. Archival availability or freeware status is not a
license. HelloWorld-PC98 has clear MIT licensing but is not a game. No suitable
existing game was validated for both rights and this profile, so the accepted
fallback is the entirely original MIT Star Trail guest.

No NEC BIOS/font, commercial DOS, period commercial game, third-party sample
artwork or unclear-rights freeware is bundled. Users can insert their own legally
obtained supported floppy images.
