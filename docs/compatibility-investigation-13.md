# Period DOS game investigation — Issue #13

Investigated locally on 2026-10-21 against checkpoint
`2eb022f0e03306a42527c51228f309eb7e4cadc3` (emulator baseline: main after
PR #12, `c1aaafa`). Source: [research Issue #13](https://github.com/takahirox/pc98-rust/issues/13).
No emulator behavior, existing fixture, dependency, CI, or publication asset changed.

**Six distinct period games executed; zero demonstrated playable.** Five fail at
identified startup stages; TRIANS98 is inconclusive because its reboot cause is
unresolved. Three distinct missing capabilities have independent original probes:
BIOS interval callbacks, GDC horizontal-blank status, and BIOS user-font loading.
The [three follow-up tasks and probe sources](compatibility-followups-13.md)
were registered as English `task` Issues with no assignee:
[interval timer #14](https://github.com/takahirox/pc98-rust/issues/14),
[horizontal blank #15](https://github.com/takahirox/pc98-rust/issues/15), and
[user font #16](https://github.com/takahirox/pc98-rust/issues/16).
Existing Issues #1/#3/#5/#7/#9/#11/#13 were checked through GitHub before
registration; no separate tickets for these defects were found.

The node instruction prohibits posting comments. Consequently, **the final
research comment on #13 was not posted**. That external completion criterion
remains pending; this checkpoint must not close #13. A proposed comment appears
below. No PR, push, merge, CI upload, or deployment was performed.

## Environment and isolation

- macOS/Darwin arm64; Rust 1.94.0; Python 3.13.13; NASM; release native core.
- Every game used `Profile::Pc9821ApDos`, original synthetic firmware, 640 KiB
  conventional RAM, the existing 4 MiB address space, 640×400 display, replacement
  Japanese font, and the unchanged deterministic 8 MHz scheduler. No proprietary
  BIOS/DOS, XMS, FEP, Windows, emulator library, or host DOS shim was used.
- All archives, unpacked files, assembled probes, DOS images and complete matching
  GPL sources, generated media, local runners, and raw diagnostic captures stayed
  under a private, mode-0700 `/tmp/pc98-issue13-local-*` directory **outside the
  worktree**. That path is ephemeral, is not a downstream dependency, and was not
  uploaded. Only original-written text and original probe source are deliverables.
- Native `Machine::step`, device/register inspection, framebuffer hashes, audio
  sample energy and guest keyboard events were used. No browser/Wasm execution,
  screenshot inspection, listening test, reference emulator, or physical-machine
  comparison was performed. Graphics-memory writes are not evidence of correct art.

The README, [hardware limits](hardware.md), [DOS boundaries](dos-compatibility.md),
[Ap devices](ap-devices.md), native runner, existing acceptance guests, fixture
preparation and publication rules were reviewed first. Star Trail, ADV98,
98VIDEOP, CPULEVEL and device fixtures are **not counted** among the six games.
The original probes below are also excluded from that count.

## Acquisition and actual author terms

The linked Vector entries are author-submitted distribution descriptions. Each
archive's original author document was inspected before execution; the freeware
category alone was not treated as a license. No game executable/data was patched,
redistributed, attached or incorporated into a public fixture.

| ID | Title / version / period | Author source and permission checked | Archive URL |
| --- | --- | --- | --- |
| SAM | Samurai Shoujoden (侍少女伝) 1.10 / 1994 | [MUTATE's own release page](https://www.mutate-soft.com/km-plus/), PC-9801 section and common notes: publisher offers these old works for free local use as boot disks with GR-DOS included | [Sam110-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/Sam110-GRDOS.zip) |
| CA | Circuit Angel 1.01a / 1994 | Same publisher authorization; PC-98 DOS release, not its Windows titles | [CircuitAngel101a-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/CircuitAngel101a-GRDOS.zip) |
| HIME | Dengeki Butouki TURBO (電撃武闘姫TURBO) 1.56a / 1993 | Same publisher authorization; original boot media includes its sound TSR and OS | [HimeT156a-GRDOS.zip](https://www.mutate-soft.com/km-plus/master/HimeT156a-GRDOS.zip) |
| MKD | MARKADIA 1.05 / 1993–1994 | [Author's Vector release](https://www.vector.co.jp/soft/dos/game/se001354.html); `MARKADIA.DOC`, use/distribution section, releases program and data as freeware; redistribution requires unchanged content and support conditions. Local execution only | [mkd105.lzh](https://ftp.vector.co.jp/01/05/3521/mkd105.lzh) |
| TRI | TRIANS98 1.02 / DOS-era release | [Toxsoft's Vector release](https://www.vector.co.jp/soft/dos/game/se011295.html); `TRIANS.DOC` chapters 9–10 remove former restrictions, declare freeware, reserve copyright and restrict redistribution/quotation. Local execution only; no document or game-content quotation published | [trian102.lzh](https://ftp.vector.co.jp/01/12/337/trian102.lzh) |
| PIPE | PIPOO 1.00 / January 1994 | [Merveilleux's Vector release](https://www.vector.co.jp/soft/dos/game/se005985.html); `PIPE.DOC` declares freeware, with archive-only redistribution, notification and sales/publication conditions. Local execution only | [pipoo100.lzh](https://ftp.vector.co.jp/00/59/3547/pipoo100.lzh) |

SHA-256 of the acquired archives (not hashes of web pages):

| ID | SHA-256 |
| --- | --- |
| SAM | `d71efe330217a88284241f8d2a93d2d3e07b6c130d05931e6d4d001aa25b3d11` |
| CA | `e03bf11b2fb491557dae0321937a541a2d118835ff4e8f671dae4e8f017cda51` |
| HIME | `9f0e4dfcc9b1f53ec7a0a518b2e28e017b68e4705040c838532d7f59ede49468` |
| MKD | `ef78e05e7140439a7b850fbe0f556602e888f508523abca91c32d57b3fc594bf` |
| TRI | `da0f682ae5a4233840485353ddeeed15fee47c81621377ec8c420a1fdedba43a` |
| PIPE | `47cd6560c892766fbe51a8db236073e1e30c8124830ac9e2d9dcadb6156f53c1` |

## Media preparation and reproduction

For SAM/CA/HIME, the publisher ZIP contains one FDI each. Validate its eight
little-endian 32-bit header fields as
`(0, 144, 4096, 1261568, 1024, 8, 2, 77)` and total length as 1,265,664 bytes.
Copy the 1,261,568-byte payload after the 4096-byte header to a **local-only** HDM.
This is a geometry-checked container conversion; it changes no disk sectors.
FDI itself is not accepted by the emulator. Boot drive 0; keep the publisher's
AUTOEXEC commands: `NL -f6 -O11` then `SS` for SAM, `NL -O11` then `CA` for CA,
and `NL -f6 -O11` then `HIME` for HIME. NL is the supplied sound TSR.

For MKD/TRI/PIPE, use the existing checksum-pinned FreeDOS(98) kernel/FreeCOM
from `tools/prepare_freedos.py`, redirecting its module-level `DEST` to a directory
outside the worktree before calling `main()`. This retains the corresponding
kernel/shell source archives and notices there. The resulting baseline
`minimal.hdm` SHA-256 was
`015faaa0dfb4c23f7a8aa4b74bbad2d460afc55c64517c5b4ea7330d4a8f8d1e`.
The four download URLs/hashes remain recorded in that preparation script; no
different guest OS was substituted.

Use `extract_fat_file` to take `KERNEL.SYS`, `COMMAND.COM`, `FDCONFIG.SYS` and
`COPYING` from that disk. Read all members of each selected game archive without
executing a host program from it, reject paths/non-8.3 names, and preserve every
file under its original name. `image_with_files` builds a fresh FAT12 floppy with
the original boot sector and those DOS/game files. Assert the rounded-up file
clusters fit the disk; the totals were 795 (MKD), 297 (TRI) and 249 (PIPE).
Add an AUTOEXEC with `@echo off`, the command below, and an original diagnostic
`echo ISSUE13 RETURNED`, using CRLF line endings:

| ID | Guest command / prerequisites | Prepared HDM SHA-256 |
| --- | --- | --- |
| SAM | Original GR-DOS boot, supplied NL TSR | `b8956ce64145f0afc79e575f5517afde1ff75d3dff152fe57387c48b36b73fe6` |
| CA | Original GR-DOS boot, supplied NL TSR; contemporary author notes discuss 386-class performance | `fd95d1b50312ba6513c03b324251f0af63d286d980b52f8b4aedf91f1ba5bdfb` |
| HIME | Original GR-DOS boot, supplied NL TSR; author notes discuss 286/386 machines | `3decb7dbdfa883256ddfff300636f41ea792156f31127082348e3836041c3f1e` |
| MKD | `MKD` batch, including supplied `MDRV98`; author requires VM-or-later 16-color PC-98, DOS, substantial free RAM; recommends faster 98s | `e2a1423f439633fb4908cec49e60342de44782946a641f02dcfce99eef3256af` |
| TRI | `TRIANS`; author requires VM-or-later 16-color PC-98 or Epson 286/386/486, DOS ≥2.11 and a writable user file | `1c0546a06cf264e5a941f02735c150fa098c838f6944dbd77ac4067f620b9f87` |
| PIPE | `PIPE`; author requires PC-98/Epson, 16 colors/two pages, DOS ≥3.3; first run generates animation and score data | `2e997f08622557737b04ad7544275829492236ea81d859ef581e41bd085a412d` |

Run `cargo build --locked --release --bin headless`, then, with a local image path:

```sh
target/release/headless /absolute/local-only/game.hdm 30000000 --ap
```

For input/state investigation a separate **local-only** Rust executable depended
on this worktree's crate by path. It inserted the image in drive 0, reset the
machine, ran 300 × `step(100_000)`, cleared collected audio after each slice, and
recorded framebuffer/text/graphics hashes and device counters every 50 slices.
It then sampled PCs for 100,000 single steps and sent Return, Space, Esc, Left,
Right, Up (PC-98 scans `1c,34,00,3b,3c,3a`). Each press lasted 50,000 slots,
followed by release and 20 × 100,000 slots. Stable cases thus received 42.4 million
slots; PIPOO stopped sooner. No execution after a core error was counted as input
coverage. A separate 35-million-slot instruction observer recorded only BIOS
parameters, port reads, reset-vector entries and local diagnostic text captures.
It made no guest-state changes or host compatibility substitutions.

## Measured matrix

| ID / genre | Classification | Launch/video/input | Audio / disk/save coverage |
| --- | --- | --- | --- |
| SAM / fighting | Fails at identified stage: NL timer initialization | GR-DOS boots and starts NL; waits with local flag zero at `108a:2130`. Timer requested at slot 1,065,590, `ES:BX=108a:1857`, `CX=2`. No title reached; graphics/frame state stable through 42.4M slots and all six keys | 119 FDC reads, zero writes; zero OPN writes/key edges and zero audio energy in initial 30M slots. Game save/load unreachable |
| CA / racing | Fails at identified stage: same NL timer initialization | Callback `1089:1857`, flag `1089:2130`, `CX=2`, requested at slot 1,068,404; same stable wait and key result | 119 reads, zero writes/audio/OPN activity; save/load unreachable |
| HIME / fighting | Fails at identified stage: same NL timer initialization | Callback `108c:1857`, flag `108c:2130`, `CX=2`, requested at slot 1,066,799; same stable wait and key result | 135 reads, zero writes/audio/OPN activity; save/load unreachable |
| MKD / vertical shooter | Fails at identified stage: horizontal-blank wait | Supplied driver installs, graphics memory changes during opening, then `2d6c:515b` repeatedly reads port `0060h`, testing bit `40h`. Status is only `04h`/`24h`; six keys do not advance the stable state | 234 reads, zero writes. At 30M slots, 5,511 OPN writes, zero key edges and zero accumulated audio energy. No claim of working music or correct opening artwork; save/load unreachable |
| TRI / falling-block puzzle | Inconclusive: repeated reboot during launch | First new firmware entry at slot 5,396,652; seven entries including initial boot within 35M slots. A reset-vector alias at `f990:66f0` (physical `ffff0`) precedes each restart. Graphics memory remains unchanged; user-file prompt/title not established | No FDC writes/OPN activity/audio energy. FreeDOS starts repeatedly. The cause could involve executable/runtime/OS or CPU behavior; no specific defect inferred and no ticket proposed |
| PIPE / pipe puzzle | Fails at identified stage: user-font BIOS call | Guest generates data and changes graphics memory before slot 26,559,171: `INT 18h/AH=1Ah`, return `3231:0f65`, `DX=7621h`, `BX:CX=3662:0018`. Core then reports `unsupported BIOS INT 18 AH=1a` | 292 reads, 33 writes, zero OPN activity/audio energy. Those writes show initialization, not a verified game save. Gameplay input, animation correctness and score/save reload were not reached |

The three MUTATE titles are independent of this emulator but share a publisher
and NL driver. They provide three titles, **one** common timer finding, not three
independent implementations of the defect. Breadth is fighting/racing/shooting/
puzzles across four author groups. No VM 8-color, PEGC 256-color, IDE, mouse or PCM
compatibility was demonstrated by this selection. No new requests for those
devices are justified here. All classifications apply only to this baseline and
the tested setup; removing one blocker does not prove a whole game will work.

## Diagnosis, controls and unresolved cases

1. **Interval timer:** the core's `INT 1Ch/AH=02h` only clears its tick word; it
   ignores `CX` and `ES:BX`. The published BIOS contract schedules a one-shot
   callback after `CX × 10 ms`. NL registers a 20 ms callback and then waits for
   its flag. The original timer probe independently prints
   `TIMER CALLBACK TIMEOUT`. Initial `018Ch=FFh` was also observed, but the
   extended sound bank is legitimately masked at reset; that read alone is not
   evidence of a sound-port defect.
2. **Horizontal blank:** `Bus::input(0060h/00A0h)` never sets bit 6. The
   [PC-98 display register reference](https://www.webtech.co.jp/company/doc/undocumented_mem/io_disp.txt)
   identifies it as horizontal blank. MARKADIA's repeated wait and the original
   `HBLANK TRANSITION TIMEOUT` probe establish a specific missing status signal.
   Other observed unmapped ports do not by themselves justify new device tickets.
3. **User font:** PIPOO requests a valid user-character code and the firmware
   explicitly rejects the service. The original font-load probe produces the
   same error without PIPOO's font data. The BIOS input pointer is **BX:CX**,
   not ES:BX. The [manufacturer's BIOS databook](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
   describes both this service (printed p.52) and the timer (printed p.13).

The exact original probe sources, local reproduction and future acceptance are
in the [follow-up record](compatibility-followups-13.md). They were assembled
locally and booted through real FreeDOS/FreeCOM. Both timeout probes returned to
DOS (headless process exit 0); the font probe stopped the core (process exit 1).
These are **baseline reproductions of failures**, not passing fix tests.

As a configuration control, the three MUTATE game executables were also launched
from fresh FreeDOS floppies without NL, preserving all other game files. All
returned to the shell without reaching a title. This driverless variant is not
a valid demonstration of normal game operation and is not another bug claim.
The unmodified publisher boot disks remain the primary reproductions.

Candidates inspected but not executed:

- Gunman 1.05 (`ganman.lzh`, Vector se023262): original `GANMAN.DOC` requires
  permission for commercial use. Permission for this implementation context was
  not assumed; skipped after document inspection.
- TADL 0.23B (`tadl023b.lzh`, Vector se001004): original `README.DOC` prohibits
  commercial use. Same conservative skip; no guest execution.
- LIFE GAME 1.00B (`life100b.lzh`, Vector se002173) and Moving Maze 1.1
  (`mmaze11.lzh`, Vector se005601): downloaded entries were source/objects or
  patch packages, not complete licensed runnable selections. Not executed or
  counted as failures; their complete-base/rights setup remains unresolved.
- Bio_100%'s old download index was inaccessible; unofficial mirrors and
  abandonware sites were not substituted as legal authority. MARKADIA was
  instead obtained from its author-submitted distribution and terms.
- MUTATE's Windows titles were excluded by platform before downloading.

## Validation and publication audit

The six games, three original probes and three configuration-control launches
described above actually ran locally. No browser or hosted game tests were added.

| Check executed | Result |
| --- | --- |
| `cargo build --locked --release --bin headless` | Passed against unchanged core |
| Build external local instrumentation crate | Passed; crate depended on this worktree by path |
| Three original NASM probes through native FreeDOS | Two expected timeout reproductions; one expected unsupported-BIOS stop, as detailed above |
| `cargo test --locked --lib --test core` | Passed: 22 core tests, zero failures/ignored tests; library has no unit tests |
| `python3 tools/build_media.py --check` | Passed; original firmware and published demo/license assets unchanged |
| `python3 tools/build_font.py --check` | Passed; replacement JIS CG hash unchanged |
| Documentation structure, relative links and whitespace | Passed; fenced source blocks match the actual locally executed original sources |
| Git scope and publication-source inspection | Only the two research Markdown files selected for commit; no changes under runtime/media/workflow/dependency paths. Pages still uploads only `web/` |

The full DOS-fixture, lint, Wasm/browser and built-site packaging suites were not
run: this checkpoint changes documentation only. The publication audit here is
an unchanged-path/content inventory and workflow inspection, not a newly built
Wasm or deployment claim.

The file deliverable consists only of this report and its follow-up record.
`git status --short`, staged paths and whitespace checks were inspected, as were
changes under `web/`, `fixtures/`, `guest/`, `src/`, `tools/`, `.github/` and
dependency manifests. The existing Pages workflow still packages only `web/`; no
download directory, raw log, game asset, OS image, probe binary or local capture
is in the commit/publication tree. The three new GitHub Issues contain only
original-written diagnostics, public provenance links and original probe source;
no external media or raw captures were published.

## Proposed final research comment (not posted)

Locally exercised six new period PC-98 DOS games on the post-PR #12 Ap profile:
Samurai Shoujoden 1.10, Circuit Angel 1.01a, Dengeki Butouki TURBO 1.56a,
MARKADIA 1.05, TRIANS98 1.02 and PIPOO 1.00. Zero are claimed playable.
The first three stop in their supplied NL driver's BIOS interval-callback wait;
MARKADIA waits for missing GDC horizontal blank; PIPOO stops on user-font BIOS
loading. Original game-free probes reproduce those three missing capabilities.
TRIANS98 repeatedly reboots, with cause unresolved and no speculative ticket.

The repository report `docs/compatibility-investigation-13.md` contains the
per-title sources, author-term checks, hashes, setup, counters and coverage gaps;
`docs/compatibility-followups-13.md` records three `task` Issues and automatable
acceptance using original probe data:
[timer #14](https://github.com/takahirox/pc98-rust/issues/14),
[horizontal blank #15](https://github.com/takahirox/pc98-rust/issues/15), and
[user font #16](https://github.com/takahirox/pc98-rust/issues/16).
Gunman/TADL were skipped for use-term ambiguity, two partial
packages were not executed, and no unofficial archive was used as permission.
All downloaded media, guest OS/source archives, generated disks and captures
remained outside the worktree; only original-written research text is committed.
No assets, raw captures or game downloads were published or sent to CI.
