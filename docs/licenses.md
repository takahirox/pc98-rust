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

The additional original `guest/hdd-ipl.asm`, `guest/mouse98.asm`,
`guest/memory98.asm`, `guest/timer.asm` and `guest/play.mml` are also MIT. Their generated DOS
fixtures include `RUST.LIC` with the repository notice. They contain no borrowed
firmware, mouse driver, sound implementation, melody or instrument sample.

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

The separate `timer.hdm` uses the same pinned, unmodified boot/kernel/shell and
configuration with the original MIT `TIMER.COM`, its `TIMER.ASM` source and
`RUST.LIC`, replacing PROBE and starting TIMER from AUTOEXEC. Its generated
`timer-manifest.json` records per-file and image hashes. Keep the same complete
FreeDOS source/license folder and preparation script with this fixture; neither
DOS fixture is part of the ten-asset Pages publication tree.

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

The original Issue #16 `fontload` control and `guest/userfont.asm` are MIT,
including their synthetic glyph patterns. `tools/prepare_font_fixture.py` builds
them on the same pinned FreeDOS fixture, alongside its sources/notices; neither
image is published. No PIPOO or other proprietary font/program data is included.

## Independent Ap DOS applications and replacement Japanese font

The new Ap DOS test fixtures use MIT ADV98 and 98VIDEOP, with original test art
and video. The release's original artwork/music and optional PMD/mouse drivers
are excluded. The exact source/license review, application behavior and limits
are in [the DOS compatibility audit](dos-compatibility.md); hashes are pinned in
[the per-title manifest](../fixtures/dos-apps-manifest.json). The generated disks
remain outside Pages and require the same GPL source-preservation obligations as
the FreeDOS fixture above. The additional FM COM compiles unmodified MIT ADV98
FM routines with an original DOS entry harness; no C runtime is linked.

The [Shinonome replacement font](../fixtures/fonts/README.md) is explicitly
redistributable and convertible under its original public-domain/non-exercise
license. Original source, AUTHORS and LICENSE are retained; generated JIS data
is embedded in Rust/Wasm. The static distribution includes that notice and author
list. No NEC font ROM or chip ROM/sample data was added.

## Original VM playable-demo selection

During the original VM work,
[ADV98](https://github.com/yarufu/pc98/blob/main/README.md) was identified as a
DOS-based 16-color engine, tested on PC-9821 systems with optional sound boards. It is not an 8-color
VM acceptance candidate. [Space Panicco's specific rights notice](https://github.com/gikonekos/Space-Panicco-Archive/blob/main/RIGHTS.md)
does not provide a clear unrestricted redistribution basis for its original
binaries, so none are copied. Archival availability or freeware status is not a
license. HelloWorld-PC98 has clear MIT licensing but is not a game. No suitable
existing game was validated for both rights and this profile, so the accepted
fallback is the entirely original MIT Star Trail guest.

No NEC BIOS/font, commercial DOS, period commercial game, third-party sample
artwork or unclear-rights freeware is bundled. Users can insert their own legally
obtained supported floppy images.

## CPULEVEL: independent public-domain DOS diagnostic

[Eric Auer's CPULEVEL archive](https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/util/system/cpulevel/cpulevel2007.zip)
contains `cpulevel.asm` and its released compressed COM. The source's first line
explicitly donates the tool to the public domain. The author asks that source
accompany the binary because it contains the documentation. The
[per-file manifest](../fixtures/cpu-manifest.json) pins the archive, original
source, original COM, uncompressed COM assembled from unmodified source, and all
files of the generated `cpu386.hdm` disk. The original downloaded ZIP is retained
beside the disk; `CPULEVEL.ASM` is also included inside the disk. No emulator or
chip implementation is fetched. The COM is a guest CPU diagnostic executed through
the real FreeDOS kernel/shell, not linked into the host.

Run `python3 tools/prepare_cpu_fixture.py` after the existing FreeDOS setup. Keep
its archive/source and the entire `fixtures/freedos` output, including complete
GPL DOS sources and notices, when redistributing this optional disk. The generated
batch/configuration and preparation script are original MIT files. CPULEVEL and
its disk remain outside `web/` and the unchanged Pages asset allowlist.

## Issue #11 HDD, music and device fixtures

The [device audit](ap-devices.md) and checked-in HDD/music/device manifests list
all selected files, exact inputs, hashes, requirements and adaptation/build steps.
The HDD uses the existing pinned GPL FreeDOS kernel and shell and an unchanged
FreeDOS `nec98/boot/boot.asm` assembled with `NEC98HDD` and `ISFAT12`. Its original
MIT partition IPL and mouse TSR have source in `guest/`. Keep complete FreeDOS
sources/notices and the original and adapted guest sources with any redistribution.
All three new images remain in ignored `fixtures/dos-apps/`, outside Pages.

PMD/MC are published guest programs from M.Kajihara (KAJA), version 4.8s,
[source commit c620dc95](https://github.com/d2lmirrors/pmd/tree/c620dc95c5e47970e7839cb5f0b7b9ab742d4f46).
The author's original UTF-8 `readme.txt` explicitly permits free use of the
published sources while retaining copyright. This is a custom permission, not
an MIT/GPL claim. Preserve that notice as `PMD-permission.txt` (also `PMD.TXT` in
the disk), the full source archive and the adapted sources/build script. Changes
only normalize include filenames, express EQU string macros as JWasm TEXTEQU,
and expand one out-of-range LOOP with a flags-preserving jump trampoline. The
music script records original and adapted source hashes and generated binaries.
No PMD guest code is translated or linked into the Rust emulator. The MC compiler
executes inside guest FreeDOS at boot, compiling the original MIT score and voice
in `guest/play.mml`; the unchanged ADV98 executable loads and starts that score.
The ADV98 release's original `PLAY.M`, effects and artwork remain excluded.

Paul Vojta's DEBUG 1.25 (2011-08-08) has an MIT permission notice in `DEBUG.ASM`;
Japheth explicitly places his extensions in the public domain. The pinned original
`DEBUG125.zip` includes its full sources/notices. Its optional startup x87 probe
is bypassed for this no-FPU profile, and the adapted source is retained alongside
the archive. Port/memory/DOS command processing is unchanged. The disk includes
the original notice-bearing source and README; no DEBUGX, DPMI examples, Win32
DLLs or unrelated archive contents are installed in the guest image.

[JWasm](https://github.com/JWasm/JWasm/tree/a5c4ea03cc0545a15d81a354251b5f534bef7a1b)
is a host assembler build tool under its upstream Sybase Open Watcom license and
notices. Its pinned full source archive and extracted source/notices remain in
the fixture output. It is not a runtime dependency and is never placed in Pages
or linked into a guest executable as a library. Only guest programs and a compiler
are acquired; no emulator/chip source or ROM is introduced. Rust dependencies
remain empty. Pages' ten-asset publication allowlist is unchanged.
