# pc98-rust

An original Rust real-mode PC-98 emulator, with a local browser demo. **Star
Trail** is a bootable guest game: its machine code draws text and bitmap VRAM,
reads PC-98 keyboard input, beeps, and saves through the emulated floppy controller.
The browser displays the Rust framebuffer and plays the core's clocked PCM.
There are no emulation dependencies, proprietary ROMs, font dumps, or server-side
execution.

The anticipated public demo is <https://takahirox.github.io/pc98-rust/> once the
GitHub Pages deployment succeeds. **Publication is currently unverified**; the
pre-merge checks below do not establish that the public URL works.

The default experimental profile targets a **PC-9801VM with 640 KiB RAM, V30 real
mode, 640×400 display and three graphics planes**. Compatibility is the subset exercised
by the included guests and the pinned FreeDOS(98) smoke test. This is not a general
PC-98 compatibility claim. See [hardware, ports, firmware and limits](docs/hardware.md).

A selectable **PC-9821Ap DOS subset** adds four-plane 16-color graphics,
replacement Japanese CG/text and approximate OPN sound. Independent ADV98 and
98VIDEOP programs now run through guest FreeDOS, with native and actual-Wasm
browser tests. The Ap profile also supports a tested 386/486 real-mode integer
subset, including 32-bit operands and addresses, SIB, FS/GS and guest faults.
The independent public-domain CPULEVEL diagnostic verifies 486 EFLAGS through DOS.
Issue #11 adds guest INT 1F extended-memory access, IDE HDD boot and persistence,
aligned EGC raster operations, PEGC packed 256-color video, bus mouse input,
sequenced ADV98 music through a guest PMD driver, and OPNA/86 PCM subsets.
See [device contracts, fixtures and unsupported features](docs/ap-devices.md).
Protected mode, DPMI, numerical FPU execution and full 486 fidelity are not implemented.
Ap provides only bounded x87 environment initialization and control/status readback.
See [guest provenance, commands and exact profile limits](docs/dos-compatibility.md).

## Build and play

Install Rust through rustup (including the Wasm target), Python 3.12+, NASM 2.16+,
Node.js 22+, a C compiler and make for reproducible guest builds and browser
testing. The application itself has no npm runtime
dependencies. Run from the repository root:

```sh
rustup target add wasm32-unknown-unknown
python3 tools/build_media.py --check
python3 tools/build_web.py
python3 -m http.server 8000 --bind 127.0.0.1 --directory web
```

Open `http://127.0.0.1:8000`, select **Play Star Trail**, then use arrows to collect
all three `*` stars with `@`. `#` cells are walls. Press **R** to restart the game.
Click the screen to give it keyboard focus. **Hello world + BEEP** boots the
independently authored MIT HelloWorld-PC98 fixture. Sound starts after a user
click; **Enable sound / Mute** controls browser audio. All runtime assets are local;
there are no network requests after the selected media loads.

**Pause / Resume** controls execution. **Reset** reboots the CPU and devices while
preserving inserted media and its writes. Select drive 1 or 2, then **Insert
media** or drop an image on the screen. Inserting drive 1 boots it; inserting drive
2 changes media without rebooting. **Eject** removes the selected disk. **Export
disk** downloads that drive's current bytes, preserving D88/HDI headers and
floppy write protection. Reinsert the export to restore saved disk data. Star Trail records the
last run's moves and score in sector 8; a new game starts at zero.
For Ap, **IDE · HDI boot** mounts a geometry-validated HDI up to 32 MiB.
Inserting IDE selects HDD boot; inserting floppy 1 selects floppy boot.
Enable **Send mouse movement and buttons** for guest mouse input; INT 33h
applications require a guest driver. The prepared HDD includes one. There is no
automatic browser storage or CPU save-state persistence.

Supported media: raw `.TFD`, `.HDM`, `.XDF`, `.2HD`, `.IMG` with exactly
77×2×8×1024, 80×2×15×512 or 80×2×8×512 bytes, and single-disk normal MFM `.D88` /
`.D98`. The latter uses validated CHRN sector headers rather than assuming fixed
sector order. Multi-disk D88, damaged/deleted sectors, arbitrary geometry and other
formats are rejected. These are **floppy disk images**, not firmware ROMs. A DOS
executable alone is not a boot disk. Personal software must fit the implemented
profile and be legally obtained; unsupported instructions/BIOS services stop with
an error instead of silently pretending to work.

The physical keyboard uses PC-98 scancodes: letters/digits, arrows, Return,
Backspace, Tab, Shift, Ctrl, F1–F10, editing keys and numeric keypad. Alt maps to
GRPH, right Alt to XFER, left Meta to NFER; the expandable on-screen PC-98 keyboard
also supplies Kana, NFER/XFER, STOP/COPY and Japanese punctuation. Key releases,
modifiers and host key repeat reach the guest. Focus loss releases held keys;
hiding the tab pauses execution. The Ap DOS profile supplies licensed replacement
Japanese CG glyphs. FEP input is not implemented.

## Reproduce the checks

```sh
python3 -m unittest discover -s tools -p test_fixture_fetch.py
# Build the independent MIT boot probes offline.
python3 tools/prepare_dma_fixture.py
python3 tools/prepare_gdtr_fixture.py
python3 tools/prepare_idtr_fixture.py
python3 tools/prepare_smsw_fixture.py
python3 tools/prepare_fpu_init_fixture.py
python3 tools/prepare_pic_fixture.py
python3 tools/prepare_pic_smm_fixture.py
python3 tools/prepare_pic_poll_fixture.py
python3 tools/prepare_pic_single_fixture.py
python3 tools/prepare_fdc_pio_fixture.py
python3 tools/prepare_fdc_write_fixture.py
python3 tools/prepare_fdc_write_multi_fixture.py
python3 tools/prepare_fdc_bios_fixture.py
python3 tools/prepare_pit_mode2_fixture.py
python3 tools/prepare_pit_mode3_fixture.py
python3 tools/prepare_pit_mode0_fixture.py
python3 tools/prepare_pit_mode4_fixture.py
python3 tools/prepare_pit_mode0_rewrite_fixture.py
python3 tools/prepare_pit_mode3_rewrite_fixture.py
python3 tools/prepare_pit_counter_latch_fixture.py
python3 tools/prepare_v30_bound_fixture.py
python3 tools/prepare_v30_repc_fixture.py
python3 tools/prepare_v30_nibble_mem_fixture.py
python3 tools/prepare_v30_segment_irq_fixture.py
python3 tools/prepare_ide_verify_fixture.py
python3 tools/prepare_rtc_fixture.py
python3 tools/prepare_rtc_set_fixture.py
python3 tools/prepare_opn_ch3_fixture.py
python3 tools/prepare_opna_irq_fixture.py
# Fetch checksum-pinned guest binaries AND matching complete source archives.
# This makes a minimal GPL guest disk; it downloads no emulator code.
python3 tools/prepare_freedos.py
python3 tools/prepare_font_fixture.py
python3 tools/prepare_dos_apps.py
python3 tools/prepare_cpu_fixture.py
python3 tools/prepare_hdd_fixture.py
python3 tools/prepare_music_fixture.py
python3 tools/prepare_device_fixture.py
python3 tools/build_font.py --check
cargo test --locked --test sound86 opna_irq_
cargo test --locked --test sound86 --test ap --test music --test pic
cargo test --locked --test ap opn_ch3_
cargo test --locked --test rtc
cargo test --locked --test timer
cargo test --locked --test pit mode4_
cargo test --locked --test pit counter_latch_
cargo test --locked --test pit mode3_rewrite_
cargo test --locked --test pit mode0_rewrite_
cargo test --locked --test pit
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
git diff --check

# Build/check the publication tree and run Chromium at / and /pc98-rust/.
npm ci
npx playwright install chromium
npm test
```

CPULEVEL and DEBUG preparation automatically try the pinned FreeDOS mirrors when
the primary download fails. Every source uses the original SHA-256 and size
checks before caching; existing cached archives are verified for offline reuse.
The canonical provenance URLs, source archives and audited image manifests remain
the same.

The original MIT [channel-3 probe](guest/opn_ch3.asm) programs each logical FM
operator through guest I/O, with keyed normal → 40h → 80h → normal stages and
Timer A stopped. Prepare offline with `python3 tools/prepare_opn_ch3_fixture.py`;
run `cargo test --locked --test ap opn_ch3_` and `npm test`. The
[checked manifest](fixtures/opn-ch3-manifest.json) pins source, boot and disk
hashes, license/build provenance, 16 PCM stages and guards. Native and production
release-Wasm tests boot the identical disk before/after reset at `/` and
`/pc98-rust/`, capture PCM every 1,000 slots, and assert pitches within
max(0.03 Hz, 0.02%), finite bounded samples, guards, unchanged media and no errors,
within 2,000,000 slots per boot. Generated assets remain outside `web/`.
Only frequency selection is implemented; CSM timer keying is absent and combined
mode `11` remains explicitly unverified. See the [hardware scope](docs/hardware.md#opnopna-channel-3-frequency-selection)
and [validation record](docs/validation.md#issue-67-opnopna-channel-3-frequency-selection-2026-10-24).

The original MIT [YM2608 IRQ probe](guest/opna_irq.asm) checks Timer A/B
suppression, pending-flag enables, retained PIC requests, actual vector-1Ch
handlers, acknowledgement/EOI and recovery on Ap. Enabled recovery holds the
shared IRQ asserted across both normal EOIs and a bounded guest delay; all eight
master/slave poll bytes must remain zero. Prepare it offline with
`python3 tools/prepare_opna_irq_fixture.py`; run
`cargo test --locked --test sound86 opna_irq_`, the full `sound86`, `ap`, `music`
and `pic` targets, and `npm test`. The
[checked manifest](fixtures/opna-irq-manifest.json) pins source, image, firmware,
assembly command and expected records/guards. Native and production release-Wasm
tests boot the same image before/after reset and at both browser roots, requiring
`OPNA IRQ PASS`, all 128 result/guard bytes, exactly four handler entries,
unchanged media and no errors within 500,000 scheduler slots per boot. Hosts only
mount, step, reset and inspect outputs. Generated files stay outside `web/`.
See the [bounded register-29 contract](docs/hardware.md#ap-ym2608-timer-irq-enables)
and [edge-routing validation record](docs/validation.md#issue-75-ap-shared-sound-irq-edges-2026-10-24).

The original MIT [RTC probe](guest/rtc.asm) exercises Ap parallel 40-bit and
serial 52-bit calendar reads, BIOS agreement and advancement with IF clear.
`python3 tools/prepare_rtc_fixture.py` builds it offline and verifies the checked
[source/boot/image/result manifest](fixtures/rtc-manifest.json). Run
`cargo test --locked --test rtc` and `cargo test --locked --test timer`; `npm test`
boots the identical disk in production release Wasm at both serving roots,
before and after reset. Each boot must report `RTC CALENDAR PASS`, match all
64 result/guard bytes and preserve disk bytes within 2,000,000 slots. Cold
construction uses 2026-01-01 Thursday; reset preserves the RTC and fractional
guest clocks. See the [hardware contract and remaining scope](docs/hardware.md#ap-calendar-read-path).

The original MIT [Time Set probe](guest/rtc_set.asm) programs both parallel and
serial calendars through ports, waits over a second while held, then releases
with commands 0/1/3 and compares literal serial/BIOS results across a month,
leap day and two-digit year transition. Build offline with
`python3 tools/prepare_rtc_set_fixture.py`; the
[checked manifest](fixtures/rtc-set-manifest.json) pins license, assembly command,
source/boot/image hashes, result layout and independent expected bytes. Native
RTC tests and production release-Wasm browser tests boot the identical image
cold and after reset at both roots, requiring `RTC SET PASS`, all 128 result and
guard bytes, unchanged media and no errors within 8,000,000 slots per boot.
Hosts only mount, step, inspect and reset; browser tests await completed mounting.
Generated assets stay outside `web/`. Time Set entry/release resets only divider
stages 10–15; the lower stages keep running. See the
[validation evidence](docs/validation.md#issue-87-ap-rtc-time-setcounter-hold).

The original MIT [IDE READ VERIFY probe](guest/ide_verify.asm) checks 40h/41h
completion and first-error task-file results in CHS and LBA modes on Ap, plus
held-input IRQ silence after poll/CPU acknowledgement and normal EOIs. Two actual
INT 11h handler entries check cascade ISR state and recovery on genuine edges. Prepare
offline with `python3 tools/prepare_ide_verify_fixture.py`; run
`cargo test --locked --test hdd ide_verify_`, `cargo test --locked --test hdd`,
and `npm test`. Native and production-release Wasm tests boot the identical HDI
before and after reset, at both browser roots, requiring guest PASS, exact
results/guards, unchanged media and zero writes within 500,000 slots per boot.
The [checked manifest](fixtures/ide-verify-manifest.json) pins the original MIT
source, synthetic firmware, image, geometry and independently calculated results.
See the [selected legacy contract and remaining IDE gaps](docs/hardware.md#ap-ide-read-verify).

The original MIT [V30 BOUND/CHKIND probe](guest/v30_bound.asm) checks signed
inclusive bounds and guest vector-5 entry with next-instruction return. It checks
exact frames, IF/TF, nonzero CS, unchanged bounds, IRET, entry counts and guards.
Prepare offline with `python3 tools/prepare_v30_bound_fixture.py`; run focused
checks with `cargo test --locked --test core v30_bound_` and
`cargo test --locked --test cpu386 bound_`. Native and production-release Wasm
tests boot the identical VM disk at both browser roots, before/after reset,
requiring `V30 BOUND PASS`, exact results/guards, unchanged disk bytes and no
FAIL/core/browser errors within two million slots. The
[manifest](fixtures/v30-bound-manifest.json) pins license, source/boot/image hashes,
assembly and expected bytes. Generated fixtures stay outside `web/`. See the
[profile-specific return contract and limits](docs/hardware.md#v30-boundchkind).

The original MIT [V30 carry-repeat probe](guest/v30_repc.asm) exercises REPC and
REPNC with CMPSB/W and SCASB/W, both segment-prefix orders, opposite incoming CF,
direction, unaligned words, zero count, early stops and exhaustion. Prepare offline
with `python3 tools/prepare_v30_repc_fixture.py`; run
`cargo test --locked --test core v30_repc_` and `cargo test --locked --test cpu386`.
Native and production-release Wasm tests at `/` and `/pc98-rust/` boot the same
image before/after reset, require guest-produced `V30 REPC PASS`, exact results,
operands and guards, unchanged disk bytes and no FAIL/core/browser errors within
two million slots. The [manifest](fixtures/v30-repc-manifest.json) pins the MIT
license, assembly command, source/boot/image hashes and expected bytes. Generated
fixtures stay outside `web/`. See the [carry-repeat contract and limitations](docs/hardware.md#v30-repcrepnc).

The original MIT [V30 segment interrupt probe](guest/v30_segment_irq.asm) checks
PIT/PIC delivery after segment MOVs in both directions, POP and consecutive
producers. Prepare offline with `python3 tools/prepare_v30_segment_irq_fixture.py`;
run `cargo test --locked --test core v30_segment_irq_` and
`cargo test --locked --test cpu386 ap_v30_segment_irq_`, then `npm test`.
The [manifest](fixtures/v30-segment-irq-manifest.json) pins MIT provenance,
assembly, source/boot/image/synthetic-firmware hashes, exact results and guards.
Native and production release-Wasm tests boot the identical VM image at both URL
roots before/after reset, requiring guest-produced `V30 SEGMENT IRQ PASS`, exact
results/guards, unchanged media and no errors within 2,000,000 slots per boot.
Generated assets stay outside `web/`. See the [bounded timing contract](docs/hardware.md#v30-segment-interrupt-inhibition).

The original MIT [V30 memory nibble-rotation probe](guest/v30_nibble_mem.asm)
checks `ROL4 mem8` and `ROR4 mem8`, including nondecimal nibbles, masked AL,
unchanged AH, complete saved FLAGS and guards. Prepare it offline with
`python3 tools/prepare_v30_nibble_mem_fixture.py`; run
`cargo test --locked --test core v30_nibble_mem_` and
`cargo test --locked --test cpu386 ap_v30_nibble_mem_`. Native and production
release-Wasm tests boot the identical VM image before/after reset at `/` and
`/pc98-rust/`, requiring guest-produced `V30 NIBBLE MEM PASS`, exact results,
unchanged disk bytes and no FAIL/core/browser errors within 2,000,000 slots.
The [manifest](fixtures/v30-nibble-mem-manifest.json) records MIT provenance,
assembly, source/boot/image hashes and exact result/guard layout. Generated media
stays outside `web/`. See the [operand subset and AL policy](docs/hardware.md#v30-memory-nibble-rotations).

The original MIT [single-mode PIC probe](guest/pic_single.asm) programs SNGL
through guest I/O, enters real PIT interrupt handlers in normal and AEOI modes,
and uses FDC READ ID to drive master IR7. It checks local vector `27h`, retained
slave IRR and empty slave ISR, no repeated entry while slave INT stays high,
separate slave polling/draining, then restored cascaded vector `53h` and separate
EOIs. Prepare offline with `python3 tools/prepare_pic_single_fixture.py`; run
`cargo test --locked --test pic` and `npm test`. The identical image boots in VM
and Ap, cold and after reset, natively and in production release-Wasm at `/` and
`/pc98-rust/`. Guest and hosts check all 64 result/guard bytes, PASS without FAIL,
unchanged media and no errors within 2,000,000 scheduler slots per boot. The
[manifest](fixtures/pic-single-manifest.json) pins its source/license, assembly
command, source/boot/image hashes, independently specified results and budget.

The original MIT [PIC boot probe](guest/pic.asm) checks programmed priority and
rotating EOIs using guest-programmed PIT and keyboard hardware interrupts. Native
and release-Wasm tests on VM and Ap at both URL roots require the exact delivery
trace `1,0,0,1,1,0`, guest-confirmed competing IRR bits, `PIC PASS`, exact results
and guards, no `FAIL` or host error, and unchanged disk bytes within two million
slots per boot, before and after reset. Its offline preparation script verifies
the checked-in [source/image/result manifest](fixtures/pic-manifest.json).

The original MIT [mode-4 PIT probe](guest/pit_mode4.asm) checks software-triggered
strobes through both port banks, delayed paired replacement, repeated count-only
arming, natural IRQ0 service with normal EOI/IRET, advancing post-terminal latched
counts and a quiet interval longer than one full wrap. Prepare it offline with
`python3 tools/prepare_pit_mode4_fixture.py`; run
`cargo test --locked --test pit mode4_` and `npm test`. Native and production
release-Wasm tests boot the identical image on VM and Ap, cold and after reset,
at `/` and `/pc98-rust/`. They require `PIT MODE4 PASS`, no `FAIL`, exact independent
32-byte results/guards, unchanged media and no errors within 2,000,000 slots per
boot; browser checks also reject page/console errors. Hosts only mount, step,
inspect and reset. The [manifest](fixtures/pit-mode4-manifest.json) pins original
MIT source, boot/image hashes and results. Generated fixtures stay outside `web/`.
Mode 4 uses held-high GATE, next-clock loads, one low clock and continued wrap
without repeated strobes. Beeper polarity follows OUT at sample endpoints; the
existing batch sampler can miss a pulse entirely between endpoints. PIT remains
partial; see [hardware scope](docs/hardware.md#8253-binary-mode-4-subset) and
[validation](docs/validation.md#issue-93-8253-binary-mode-4-2026-10-24).

The original MIT [mode-0 PIT probe](guest/pit_mode0.asm) arms a terminal-count
IRQ0 twice using fresh mode words. Each arming verifies one hardware entry with
EOI/IRET, advancing latched post-terminal counts and no additional IRQ during a
continued full counter wrap. Guest waits are bounded. Native and release-Wasm
tests boot the identical image on VM and Ap before/after reset at both browser
roots, requiring exact results/guards, `PIT MODE0 PASS` without `FAIL` or errors,
and unchanged disk bytes within two million slots per boot. The offline
[manifest](fixtures/pit-mode0-manifest.json) pins source/image hashes, license and
exact guest-owned results. See the [mode-0 contract](docs/hardware.md#8253-binary-mode-0-subset).

The separate original MIT [mode-0 rewrite probe](guest/pit_mode0_rewrite.asm)
programs the mode once, interrupts an active count with a delayed paired write,
then rearms after terminal using count writes alone. Each delayed pair must
suppress IRQ0 for over a full counter wrap; each completed replacement must
deliver exactly one IRQ0 with ISR/EOI/IRET and no repeats across later wraps.
Native and production-release Wasm tests use the identical image on VM/Ap before
and after reset, at both browser roots, with the same two-million-slot maximum,
exact results/guards, PASS without FAIL or errors, and unchanged disk bytes.
The offline [manifest](fixtures/pit-mode0-rewrite-manifest.json) pins source,
image, license and results; preparation retains all provenance beside the image.

The original MIT [mode-2 PIT probe](guest/pit_mode2.asm) latches a running
60000-count period, rewrites its divisor to 2000, and verifies that the live count
continues the old period. It discards setup transitions by reinitializing the PIC,
then services two periodic hardware IRQ0 entries with EOI/IRET and confirms the
new period's count. Native and production-release Wasm tests run the identical
image on VM and Ap before/after reset, at both browser roots. Each boot requires
`PIT MODE2 PASS`, exact guest-owned results/guards, unchanged disk bytes and no
error or `FAIL` within two million slots. Its offline
[manifest](fixtures/pit-mode2-manifest.json) pins hashes, license and result layout.

The original MIT [mode-3 PIT probe](guest/pit_mode3.asm) checks two latched
counts from a running 60000-count divisor to distinguish decrement-by-two from
the old countdown. It programs a new mode word with divisor 2000, discards setup
edges by initializing the PIC, then checks two periodic hardware IRQ0 entries
with EOI/IRET. Native and production-release Wasm tests boot the identical image
on VM and Ap before/after reset, at both browser roots, requiring exact results
and guards, `PIT MODE3 PASS` without `FAIL` or errors, and unchanged disk bytes
within two million slots. Its offline
[manifest](fixtures/pit-mode3-manifest.json) pins hashes, provenance and results.

The separate original MIT [mode-3 rewrite probe](guest/pit_mode3_rewrite.asm)
programs the mode once, then replaces 60000 with 2000 using count bytes alone.
A subsequent latch must still show the old half-cycle counting down, which
rejects an immediate restart. Two natural IRQ0 entries check the new count,
ISR, EOI and IRET. Native and production-release Wasm tests use the identical
image on VM/Ap before and after reset, at both browser roots. Each boot requires
exact results/guards, `PIT MODE3 REWRITE PASS` without FAIL or errors, unchanged
disk bytes and at most two million slots. The offline
[manifest](fixtures/pit-mode3-rewrite-manifest.json) pins source/boot/image hashes,
assembly command, license, expected results and budget; the preparation script
retains source, license and manifest beside the image.

The separate original MIT [counter-latch probe](guest/pit_counter_latch.asm)
checks repeated ordinary commands before any read and between paired bytes,
then recaptures the running count after each completed snapshot. Commands and
reads cross the primary and mirrored banks. Native and production-release Wasm
tests boot the same image on VM/Ap before and after reset, at both browser roots,
requiring `PIT COUNTER LATCH PASS`, no FAIL/core/browser errors, exact raw result
bytes/guards and unchanged disk bytes within two million slots. Its offline
[manifest](fixtures/pit-counter-latch-manifest.json) pins assembly, license,
source/boot/image hashes, expected bytes and budget. Prepare it with
`python3 tools/prepare_pit_counter_latch_fixture.py`; run focused native coverage
with `cargo test --locked --test pit counter_latch_`.

The original MIT [polling probe](guest/pic_poll.asm) uses CLI and real port
instructions to acknowledge guest-programmed PIT and FDC READ ID requests,
checks separate master/slave ISR and EOI results and empty polls, then verifies
ordinary PIT IRQ delivery after STI. Prepare it offline with
`python3 tools/prepare_pic_poll_fixture.py`; run `cargo test --locked --test pic`
and `npm test`. Native and release-Wasm browser checks require `PIC POLL PASS`,
all 32 exact result/guard bytes and unchanged disk bytes within 2,000,000 slots,
in VM and Ap before/after reset and at both browser roots. The
[manifest](fixtures/pic-poll-manifest.json) pins provenance and source/boot/image
hashes. This verifies the bounded [normal-EOI polling contract](docs/hardware.md#8259a-ocw3-normal-eoi-polling);
PIC remains partial.

The separate original MIT [special-mask probe](guest/pic_smm.asm) enters a
keyboard handler with a confirmed timer request pending and IR0 lowest. It
demonstrates normal-mode blocking while IF is set, then enables OCW3 special-mask
mode to nest the timer. Both profiles run the identical image natively and in
release Wasm at both URL roots, before and after reset. Each boot requires
`PIC SMM PASS`, trace `01 00 01`, ISR snapshots `02 03 02`, timer counts zero
before enabling and one afterward, exact guards, unchanged disk bytes, no error
or `FAIL`, and completion within two million slots. Its offline
[manifest](fixtures/pic-smm-manifest.json) pins provenance, hashes and results.

The original MIT [x87 boot probe](guest/fpu_init.asm) checks FNINIT and
non-waiting control/status readback, fixed two-byte stores, prefixes, guards and
guest exception repair. Prepare it offline with
`python3 tools/prepare_fpu_init_fixture.py`; run
`cargo test --locked --test fpu --test cpu386 --test core` and `npm test`.
Native and production release-Wasm browsers boot the same
[hash-pinned image](fixtures/fpu_init-manifest.json) cold and after reset at both
URL roots within 2,000,000 slots. See the [bounded x87 contract](docs/hardware.md#ap-x87-environment-initialization).
FPU remains partial; numerical execution and WAIT/NE/IRQ behavior remain required.

The original MIT [SMSW boot probe](guest/smsw.asm) checks CPU-local reset status,
register upper halves, exact two-byte stores, addressing and guest-handled
restartable faults. Native and production-Wasm tests at both URL roots require
`SMSW PASS`, exact results and guards, no `FAIL` or host error, and unchanged disk
bytes within two million scheduler slots, before and after reset. Its
[manifest](fixtures/smsw-manifest.json) pins source/image hashes and provenance.
See the [bounded SMSW contract](docs/hardware.md#ap-real-mode-smsw).

The original MIT [IDTR boot probe](guest/idtr.asm) checks Ap LIDT/SIDT,
relocated real-mode vectors, restartable operand faults, inclusive table limits
and terminal shutdown. Run `cargo test --locked --test cpu386 idtr_` for the
focused native checks. Production-Wasm browser tests execute the identical
probe at both paths, before and after reset, checking exact bytes, guards,
shutdown and unchanged disk data within two million slots per run. See its
[manifest](fixtures/idtr-manifest.json) and the [implemented boundary and
specification discrepancies](docs/hardware.md#ap-real-mode-idtr).

The original MIT [GDTR boot probe](guest/gdtr.asm) checks Ap real-mode
LGDT/SGDT, exact load/store bytes and guards, reset readback and restartable
faults. Native `cargo test --locked --test cpu386 gdtr_` and production-Wasm
browser tests run the same image before and after reset, at both serving paths,
within two million slots per run. Its [manifest](fixtures/gdtr-manifest.json)
records source/license, assembly and hashes. This adds only a
[GDTR prerequisite](docs/hardware.md#ap-real-mode-gdtr); protected mode remains
unimplemented. Generated test fixtures stay outside `web/`.

The original MIT [DMA auto-init boot probe](guest/dma_autoinit.asm) builds into
`fixtures/dma/autoinit.hdm`, separate from all existing disks and publication assets.
`cargo test --test dma` and production-Wasm browser tests boot it on VM and Ap,
with a two-million-slot budget at each serving path. It programs raw 8237/FDC
ports, reuses the DMA buffer for two reads and two writes, and checks register
reload, exact bytes, guards, result phase and completion IRQ. A subsequent
master-clear phase checks retained readback, masked no-transfer commands, and
successful raw FDC read/write after unmasking without DMA reprogramming. It emits
an explicit `DMA MASTER CLEAR PASS` or `DMA MASTER CLEAR FAIL` result. The
controller-disable phase then blocks unmasked raw reads and writes, checks
retained programming, status and guards, and enables fresh transfers without DMA
reprogramming. It adds `DMA DISABLE PASS` / `DMA DISABLE FAIL` markers while
preserving the earlier phases and their assertions. The
[manifest](fixtures/dma-manifest.json) records the assembly command, license and
source/image hashes. No DOS, proprietary ROM or game is required. See the
[bounded DMA contract](docs/hardware.md#dma-terminal-count-and-auto-initialization).

The original MIT [non-DMA WRITE DATA probe](guest/fdc_write.asm) builds offline
with `python3 tools/prepare_fdc_write_fixture.py`. Run `cargo test --locked --test
fdc --test fdc_bios --test core --test dma --test pic` and `npm test`. Native and
production release-Wasm VM/Ap boot the identical image before/after reset at `/`
and `/pc98-rust/`, within two million slots. Guest interrupt handlers supply the
single-sector payload and validate readback, results, PIC records and guards;
hosts require `FDC WRITE PASS`, exact whole-media changes and no errors. The
[manifest](fixtures/fdc-write-manifest.json) pins source/license, assembly,
firmware/image hashes and independent expectations. See the
[write subset and prefix-retention policy](docs/hardware.md#fdc-non-dma-write-data).

The separate original MIT [multi-sector write probe](guest/fdc_write_multi.asm)
builds offline with `python3 tools/prepare_fdc_write_multi_fixture.py`. The same
native/browser commands run it on VM and Ap before/after reset at both serving
paths. One R=2/EOT=3 command writes distinct sectors, then one PIO command reads
both back. The guest services exactly 2,048 byte interrupts per direction, one
result interrupt per command, and both PIC EOIs; it checks records, guards and
restored SP before `FDC WRITE MULTI PASS`. Hosts compare all records, payloads and
complete exported media within 2,000,000 slots. The
[manifest](fixtures/fdc-write-multi-manifest.json) pins provenance and independent
expectations. See [Issue #85 validation](docs/validation.md#issue-85-single-head-multi-sector-fdc-non-dma-write-data).

The original MIT [non-DMA FDC probe](guest/fdc_pio.asm) builds offline with
`python3 tools/prepare_fdc_pio_fixture.py`. Run `cargo test --locked --test fdc`
and `npm test`. VM and Ap boot the identical floppy before/after reset, natively
and in release Wasm at both serving paths. Its actual INT 13h handler distinguishes
2,048 data interrupts from the separate result interrupt, with DMA masked and
disabled. Guest and hosts require exact payload, seven results, guards, unchanged
media and zero writes within two million slots per boot. The checked
[manifest](fixtures/fdc-pio-manifest.json) pins original source, assembly command,
synthetic firmware/image hashes and expectations. See the
[implemented subset and timing policy](docs/hardware.md#fdc-non-dma-read-data).

Prepare the original MIT BIOS floppy DMA handoff probe with
`python3 tools/prepare_fdc_bios_fixture.py`.
Run `cargo test --locked --test fdc --test fdc_bios` and `npm test`. VM and Ap
boot the identical image natively and through production release Wasm at both
serving paths, before/after reset. The guest issues actual INT 1Bh reads/writes
after fresh SPECIFY ND=1, reads back both writes, and completes a PIO read → BIOS
transfer → fresh SPECIFY ND=1 → PIO read sequence. Each boot requires
`FDC BIOS PASS`, seven successful CF/AH pairs, exact payloads and guarded RAM,
exactly two intended sector writes with a complete media comparison, and no
guest/core/browser errors within 2,000,000 slots. The
[manifest](fixtures/fdc-bios-manifest.json) pins original source, license,
assembly command, hashes and independent expected results. Generated media stays
outside `web/`. See the [handoff contract](docs/hardware.md#bios-floppy-dma-handoff)
and [Issue #81 validation](docs/validation.md#issue-81-bios-floppy-dma-handoff).

`npm run build` checks reproducible media, compiles the real Rust/Wasm core and
runs `npm run check:site`. The latter verifies a nonempty, compilable Wasm module
with the emulator exports and an exact list of the ten distributable assets,
including both approved disks, MIT notices and the replacement font notice.
Unexpected files, directories or symlinks fail the check. `npm test` runs the same browser acceptance suite at the
local root and at a simulated Pages project subpath whose root asset URLs fail.
To repeat just the project-path browser checks after building, run
`npx playwright test --project=pages`.

`cargo test --test dos`, `cargo test --test ap` and `cargo test --test cpu386`
deliberately fail if their legal guest fixtures are missing; they are never
silently skipped. The fixture downloads require network access once.
Their checksums and sources are pinned in the preparation script. **PROBE.COM**
runs via the guest FreeDOS kernel and shell, including real INT 21h handling inside
DOS. The emulator has no host DOS implementation. The minimal fixture excludes
XMS drivers because this profile is real-mode V30, not a 286 machine. To try it in
the browser, insert `fixtures/freedos/minimal.hdm`, wait for `A:\>`, and type `probe`.
See [licensing and distribution](docs/licenses.md) before redistributing this disk.
The same preparation also builds `fixtures/freedos/timer.hdm` from the original
MIT `guest/timer.asm`. It boots TIMER.COM through FreeDOS/FreeCOM and validates
the BIOS interval callback's flag, interrupt stack and return address before
printing `TIMER CALLBACK PASS`. The guest stays in an observation loop so native
and actual-Wasm browser tests can confirm that it fires only once in both
profiles. See the [interval service contract](docs/hardware.md#bios-interval-callback-int-1cah02).
For the independent DOS game/video/audio tests and CPULEVEL diagnostic, select
the Ap DOS machine and insert the generated `fixtures/dos-apps/*.hdm` disks; see [instructions](docs/dos-compatibility.md).

The same preparation builds `fixtures/freedos/keymap.hdm` with the exact original
MIT [Issue #25 keyboard probe](guest/keymap.asm). `cargo test --test keyboard`
and actual-Wasm browser tests boot it through FreeDOS on both profiles, hold
Return and release it through ordinary input, then assert `KEYMAP DOWN PASS`,
`KEYMAP UP PASS` and shell return within seven million scheduler slots. Native
tests also cover all 16 groups, simultaneous keys, repeats, break codes, IRQ
gating, guest interrupt hooks, full-ring behavior and reset. See the
[BIOS held-key contract](docs/hardware.md#bios-held-key-bitmap). No game or ROM
is required, and this DOS fixture stays outside the Pages publication tree.

`prepare_freedos.py` also builds `fixtures/freedos/hblank.hdm` from the original
MIT [horizontal-blank probe](guest/hblank.asm). `cargo test --test gdc` requires
this disk and verifies `HBLANK TRANSITION PASS` through guest DOS on both profiles,
within 16 million scheduler slots. Actual-browser tests run the same disk and
budget on real Wasm at both serving paths. The probe initializes video and polls
both `0060h` and `00A0h`; no game or ROM download is needed. See the
[fixed display timing contract](docs/hardware.md#device-ports-and-interrupts).

The same FreeDOS preparation builds `fixtures/freedos/lio.hdm` with the original
MIT [LIO boot-loop control](guest/lioinit.asm) and [GINIT contract test](guest/liotest.asm).
`cargo test --test lio` requires this disk. Native tests check ROM registration,
work-area guards, palette/page defaults, repeated calls, reset and rendered pixels;
they observe one initial reset and AH=00h returns during 16 million slots.
Production-Wasm browser tests exercise the same DOS programs and pixels on both
profiles and serving paths. See the [GINIT boundary](docs/hardware.md#graph-lio-directory-and-ginit);
later LIO commands are explicitly unsupported. The disk is outside publication.

The same preparation builds `fixtures/freedos/drawmode.hdm` (the exact Issue #23
control) and `drawtest.hdm` (an original register/state/switching probe).
`cargo test --test drawing_timing` and the browser suite execute them through
FreeDOS on Ap, including return to the shell and reset. See the
[drawing timing contract and limits](docs/hardware.md#bios-drawing-timing-int-18ah4a).

For a native/headless run of the same core:

```sh
cargo run --release --bin headless -- web/media/hello98.tfd 100000
cargo run --release --bin headless -- web/media/startrail.hdm 200000
cargo run --release --bin headless -- fixtures/freedos/minimal.hdm 5000000
```

The optional third headless argument enables bounded instruction tracing near the
end of the requested budget. Native callers can use `Machine::step`, `bus.key`,
`insert`, `reset`, `render`, registers, memory, text, IRQs and media directly.
Each step call is capped at 100,000 scheduler slots; REP instructions perform one
iteration per slot. Rebuild checked-in guest artifacts with
`python3 tools/build_media.py` after editing assembly sources. The generated
`web/core.wasm` is build output and is not committed.

The original [user-character BIOS controls](docs/user-characters.md) also boot
through this legal FreeDOS fixture and compare text/graphics pixels in native
and browser Wasm tests.

[CI](.github/workflows/emulator.yml) performs the fixture preparation, native
checks, Wasm build and browser tests on pull requests before merge. Local test
coverage and execution evidence are in [validation](docs/validation.md). No
published site, post-merge deployment, human playtest or original hardware is
required for pre-merge acceptance. No protected mode, SCSI, NEC BASIC, proprietary
font, complete EGC/PEGC or complete OPNA fidelity is claimed.
Unsupported CPU/BIOS operations report errors;
unmapped ports return an open bus and are recorded for headless diagnostics.

## GitHub Pages deployment

[`.github/workflows/pages.yml`](.github/workflows/pages.yml) rebuilds and deploys
on **every push to `main`**, including merges. It also supports manual reruns via
Actions → GitHub Pages → Run workflow on `main`; dispatches from other branches
are skipped. Pull requests run the build, packaging check and browser tests in
the existing [acceptance workflow](.github/workflows/emulator.yml), with no public
deployment or Pages write permission.

The Pages build installs the Wasm target and NASM, runs `npm ci` and
`npm run build`, then uploads **only `web/`** with the official
`actions/upload-pages-artifact` action. `actions/configure-pages` and
`actions/deploy-pages` publish that artifact in a separate job with Pages write
and OIDC permissions. The `github-pages` environment records the output URL;
deployments are serialized and have no manual reviewer gate. Generated
`web/core.wasm` stays gitignored. No generated branch, backend, proprietary ROM,
emulator library, `target/` or `fixtures/freedos/` is published. The two cleared
demo disks and their license notices are included; optional GPL test fixtures
remain outside the distributable. HTML, script, styles, Wasm and media references
are relative, so local serving and `/pc98-rust/` use the same files.

The repository setting must be **Settings → Pages → Build and deployment →
Source: GitHub Actions** (`build_type: workflow` in the API). This initial setup
was applied for Issue #5; see the [configuration and validation record](docs/pages-validation.md).
The `github-pages` environment permits `main` with no required reviewers. If the
site is disabled later, an authenticated administrator can restore the Actions
source through Settings or the [Pages REST API](https://docs.github.com/en/rest/pages/pages):

```sh
gh api repos/takahirox/pc98-rust/pages --jq '{build_type,html_url,status}'
# Only when the site is disabled (GET returns 404):
gh api --method POST repos/takahirox/pc98-rust/pages -f build_type=workflow
# For an existing site that uses branch publishing instead:
gh api --method PUT repos/takahirox/pc98-rust/pages -f build_type=workflow
```

The default Actions `GITHUB_TOKEN` cannot always enable a disabled site.
`configure-pages` therefore does not request automatic enablement; that option
requires additional repository administration/Pages privileges. Keep all admin
credentials outside code, logs and the repository. If setup is denied, record the
API status and missing permissions and use the exact Actions-source setting above
as operational follow-up; it is not a human approval or pre-merge gate.

For failures, inspect the GitHub Pages workflow's failed step. Reproduce a build
or packaging failure with `npm ci && npm run build`; install NASM and the Wasm
target as described above. Reproduce runtime/path issues with `npm test`.
Missing `core.wasm` or a stale disk must be fixed by rebuilding, not by committing
generated Wasm. For configure/deploy errors, confirm `build_type: workflow`, the
job's Pages/OIDC permissions and that the environment allows `main` without
required reviewers. A successful local build does not prove publication.

**Required post-merge operational follow-up for Issue #5 (pending):** check that
the merge's `main` push starts the Pages workflow, inspect its result and verify
the public project URL loads real Wasm, Star Trail and keyboard/audio behavior.
If deployment, source enablement, HTTP access or runtime behavior fails, create a
new `task`-labeled Issue with the run URL, error and reproduction and address it
independently. This follow-up does not block review approval, Issue closure or
merge. Until it is performed successfully, report publication as unverified.

## Attribution and contributing

Original emulator, firmware, glyphs and Star Trail / PROBE sources are MIT licensed.
The hello-world boot sector and its source retain their separate MIT notice.
The downloaded FreeDOS/FreeCOM guests are GPLv2 or later and keep corresponding
source archives beside the generated image. See [file-level audit](docs/licenses.md).

- [Development flow and English collaboration convention](docs/development-flow.md)
- [Review and merge guidelines](docs/review-guidelines.md)
- [Issue template](.github/ISSUE_TEMPLATE/issue.md) and [PR template](.github/pull_request_template.md)
- [AI agent instructions](AGENTS.md)
