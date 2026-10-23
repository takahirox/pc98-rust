# Validation evidence

## Issue #25 BIOS held-key bitmap validation (2026-10-21)

The INT 09 firmware now consumes each ready keyboard port byte, updates only its
scan-code bit at `0000:052A–0539`, and retains character-ring/modifier processing
and PIC EOI. INT 18h/AH=04h reads that same table. Pending host input cannot bypass
IRQ delivery or a guest DOS/TSR handler. The [keyboard contract](hardware.md#bios-held-key-bitmap)
specifies grouping, make/break ordering, ring limits and the existing fresh-device
reset policy. The layout and BIOS reference pages are linked from Issue #25;
direct PDF retrieval returned HTTP 503 during this run, so no new claim about
physical hardware reboot behavior is made.

The following checks passed in the assigned macOS arm64 worktree:

| Exact command | Result |
| --- | --- |
| `python3 tools/prepare_freedos.py` | Passed; minimal image unchanged; new keymap disk, exact MIT source/COM and complete pinned DOS sources/notices retained |
| `python3 tools/prepare_font_fixture.py` | Passed; existing original font controls |
| `python3 tools/prepare_dos_apps.py` | Passed; pinned application fixtures unchanged |
| `python3 tools/prepare_cpu_fixture.py` | Passed; pinned CPULEVEL fixture |
| `python3 tools/prepare_hdd_fixture.py` | Passed; pinned IDE/mouse fixture |
| `python3 tools/prepare_music_fixture.py` | Passed; pinned PMD music fixture |
| `python3 tools/prepare_device_fixture.py` | Passed; pinned DEBUG/EGC/PEGC fixture |
| `python3 tools/build_font.py --check` | Passed; replacement font unchanged |
| `cargo test --locked` | Passed; all 87 native integration tests, including seven new keyboard tests |
| `cargo test --locked --test keyboard` | Passed on restored final firmware after the negative control described below |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` and `npx playwright install chromium` | Passed |
| `npm test -- --config=.issue25-playwright.config.js` | Passed; reproducible media/font, release production Wasm, ten-asset publication audit, all 38 Chromium cases at root and project paths in 36.9 seconds |
| `git diff --check` | Passed |

Unmodified `npm test` first reached a successful build/publication audit but could
not bind the occupied local port 8000. The temporary configuration above imported
the normal Playwright configuration and changed only server ports/base URLs to
61080/61081; its project server retained the same root-asset rejection. Both
temporary files were removed afterward. No running host service was stopped and
no permanent test/CI configuration was changed.

The exact Issue control is preserved in `guest/keymap.asm`, source SHA-256
`833f8925cf99c2cbbcce5f05fd3d7a1c129ab229276203a7661eb4ad497440f5`.
The preparation script checks that source hash and records all image/file hashes.
The new `keymap.hdm` SHA-256 is
`67848b321ea6ad1a96811bfab90243e1436abb9a605abffdb2a9d8dc8cfae0f6`.
Both VM and Ap boot it through the real FreeDOS kernel and FreeCOM for five
million slots, hold Return for one million and release for one million. Native
and production-Wasm browser tests require both pass messages, absence of
`KEYMAP BWA STALE`, a cleared bitmap and `PROBE SHELL RETURN` plus the DOS prompt.
Browser tests use ordinary Enter down/up events at both serving paths and verify
that the bitmap changes only after stepping the queued interrupts.

As a negative control, the original checkpoint's `src/firmware.rs` was temporarily
restored and the original-probe native test failed with `KEYMAP BWA STALE` as
expected. The fixed firmware was restored immediately and all seven keyboard
tests, Clippy and formatting were checked again. An early test-only expectation
of exactly two guest hook entries was corrected to account for FIFO-ready IRQs
latched during the guest prologue; empty BIOS entries must add no phantom key.

The native cases exercise actual guest INT/IRET, CPU/PIC/ROM IRQ delivery, all 16
groups, simultaneous holds including modifiers, raw break codes, queued makes
and breaks, repeats, IF/mask gating, full/peek/get/flush ring behavior, reset and
guest hooks that chain or consume input. A full-memory comparison restricts
serviced Return writes to its bitmap byte, the existing ring and interrupt stack.
All prior keyboard, DOS, timer, application, video, audio and persistence checks
pass. No game, proprietary ROM/font or new publication asset is required. Optional
MARKADIA/MUTATE retests were not performed; these results establish the keyboard
contract, not game playability or music correctness. No post-merge verification
is required by Issue #25.

## Issue #14 interval callback validation (2026-10-21)

Implemented the normal-mode INT 1C/AH=02 single-event service from the
[manufacturer BIOS databook, printed p.13](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
and [programmer reference, printed p.52](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf).
The original PDFs were inspected locally; no other emulator implementation was
used. The [service contract](hardware.md#bios-interval-callback-int-1cah02)
documents timing, IRQ delivery, register preservation, reset and the deliberate
single-slot replacement policy. The references do not define overlapping AH=02
requests; replacement is an explicit emulator choice, not a claimed quotation.

The following checks passed in the assigned macOS arm64 worktree:

| Exact command | Result |
| --- | --- |
| `python3 tools/prepare_freedos.py` | Passed; existing minimal image unchanged; original MIT TIMER source/COM and matching DOS sources/notices prepared |
| `python3 tools/prepare_dos_apps.py` | Passed; pinned baseline application images unchanged |
| `python3 tools/prepare_cpu_fixture.py` | Passed; pinned CPULEVEL fixture |
| `python3 tools/prepare_hdd_fixture.py` | Passed; pinned IDE/mouse fixture |
| `python3 tools/prepare_music_fixture.py` | Passed; pinned guest PMD music fixture rebuilt |
| `python3 tools/prepare_device_fixture.py` | Passed; pinned DEBUG/EGC/PEGC fixture rebuilt |
| `python3 tools/build_font.py --check` | Passed; replacement font unchanged |
| `cargo test --locked` | Passed; all 64 existing tests and the seven timer tests then present |
| `cargo test --locked --test timer` | Passed after adding the eighth timer test; all eight pass, covering 72 distinct native integration tests across the two runs |
| `cargo clippy --all-targets -- -D warnings` | Passed on final Rust sources |
| `cargo fmt --check` | Passed on final Rust sources |
| `npm ci` | Passed; existing development dependencies unchanged |
| `npx playwright install chromium` | Passed |
| `npx playwright test --grep 'DOS timer callback'` | Passed; four actual-Wasm cases, both profiles at both serving paths |
| `npm test` | Passed in 40.0 seconds: reproducible media/font checks, release Wasm build, exact ten-asset publication audit, all 28 Chromium tests at `/` and `/pc98-rust/` |
| `git diff --check` | Passed |

Native tests verify no early callback for CX=1/2/17, CX=0 meaning 655.36 seconds,
exactly one guest entry, FLAGS/CS/IP frame contents, SP and general/segment
register preservation, IRET resumption, IF/PIC priority/mask/STI gating, replacement
of active and expired requests, callback rearming and reset before/after expiry.
The eighth test verifies that frequent PIT IRQs cannot fire the callback early and
that one-slot, 37-slot and large stepping slices produce identical final state.
Repeated stepping and large virtual-clock advances do not retrigger a consumed
request. Scheduling and delivery use bounded state and the existing CPU/PIC path.

`guest/timer.asm` is an original MIT control based on the Issue #13 probe.
`fixtures/freedos/timer.hdm` boots through the actual pinned FreeDOS kernel and
FreeCOM and prints `TIMER CALLBACK PASS` within a 16-million-slot budget in both
profiles, natively and in real browser Wasm. The guest itself captures and checks
the interrupt SP, return IP/CS and saved IF before setting a distinct result byte.
Native and browser tests also inspect that initialized resident state, verify its
location against the guest segment, and run another million slots with the flag
still exactly one. The observation loop intentionally retains the guest memory.
An initial fixture state signature had nine bytes while assertions assumed eight;
it was corrected and both final native and browser checks pass.

Timer image SHA-256:
`5b49fc36702b8f23e4b76ae641c5ba95c20ce4b7091538b4d41dee1221e60ded`.
The generated per-file manifest, original source/license and complete pinned DOS
source/notices stay with the ignored fixture. Pages still includes only the ten
approved assets; neither DOS image is published.

All existing VM/Ap timing, music/audio, input, floppy/IDE persistence, graphics,
CPU and publication tests pass. No MUTATE media, proprietary BIOS or commercial
DOS was needed or downloaded. Optional publisher-game boots were not performed;
these results do not establish complete game compatibility. No push, PR, merge,
comment, deployment or post-merge action was performed. Issue #14 requires no
post-merge verification.

## Issue #11 device validation (2026-10-21)

The following commands passed in the assigned macOS arm64 worktree. Native Rust
was Homebrew 1.94.0; release Wasm used rustup stable 1.99.0, consistently with the
existing build helper. The initial direct Wasm command used Homebrew's compiler,
which lacked the target library; selecting rustup's compiler resolved that local
toolchain mismatch. No application dependency or target workaround was added.

| Exact command | Final result |
| --- | --- |
| `python3 tools/prepare_freedos.py` | Passed; pinned minimal image and complete GPL source/notices retained |
| `python3 tools/prepare_dos_apps.py` | Passed; baseline ADV98/98VIDEOP/FM image and per-file hashes unchanged |
| `python3 tools/prepare_cpu_fixture.py` | Passed; CPULEVEL archive/source/media match committed manifest |
| `python3 tools/prepare_hdd_fixture.py` | Passed; HDI `5962d17c914b8fd5abd2b1b692546b111448514770adb1f775eaa9c0670bed1f` |
| `python3 tools/prepare_music_fixture.py` | Passed; published guest driver/compiler rebuilt, image `2dc81c9278af02392348fb43f23a729f5e0ada918d4c9d9c5d5e0df083bac37c` |
| `python3 tools/prepare_device_fixture.py` | Passed; DEBUG adaptation/build and probe image `dff11e9e5acc3d8a98a26c0429f6aac69c7824441a4d055a8e1eb8dcfe90f51d` |
| `python3 tools/build_media.py --check` | Passed; new HDD-capable synthetic IPL reproduced; both published demo disks/notices unchanged |
| `python3 tools/build_font.py --check` | Passed; 282752-byte Shinonome mapping unchanged |
| `cargo test --locked` | Passed: 64 integration tests, no ignored tests: 22 core, 21 CPU386, 8 Ap, 5 device, 4 HDD/mouse, 2 sound86, 1 FreeDOS, 1 game music |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `RUSTC="$(rustup which rustc)" cargo build --locked --release --target wasm32-unknown-unknown --lib` | Passed; explicit compiler selection matches `tools/build_web.py` |
| `cargo tree` | Only pc98-rust; no Rust dependencies |
| `npm ci` | Passed; unchanged development-only dependencies |
| `npx playwright install chromium` | Passed |
| `npm test` | Passed: release Wasm build, exact ten-asset publication audit, 24 actual-Wasm Chromium tests, 12 at each serving path; final browser run 36.6 seconds |
| Relative documentation links, JSON manifest parsing, `git diff --check` | Passed |

Production Wasm was 437981 bytes. Acceptance checks inspect real guest memory,
VRAM/palettes, DOS FAT file chains, controller transactions and clocked audio.
The HDD test verifies guest-created `SAVE1.DAT` itself, rather than searching for
a signature also present inside the application executable. Pointer events reach
the emulated PPI, IRQ13 and resident guest driver before changing ADV98's UI.
The game music test proves the title is silent and the independent game initiates
sequenced PMD FM/SSG output; a separate Web Audio test covers 98VIDEOP's 86 PCM.

Review against Issue #11: the selected guest requirements now have independent
DOS execution evidence for HDD, mouse, 256-color output and game music, plus
targeted original tests for memory/device boundaries. CPU/VM/Japanese/floppy and
deployment regressions remain in the same suites. No host DOS/mouse/music API,
application-name workaround, emulator dependency, proprietary ROM or guest asset
was introduced. The [device audit](ap-devices.md) explicitly limits EGC/PEGC,
sound, memory services and driver coverage to their supported subsets; unsupported
physical features must not be inferred from these results. There is no claim of
arbitrary game compatibility or a complete physical Ap model.

No push, PR, merge, GitHub comment, live deployment or post-merge action was
performed. Source and notice obligations remain reproducible before merge.

## Historical Issue #3 pre-merge validation

The implementation is an original, limited VM-family real-mode emulator. Its
acceptance guests are the cleared independent hello-world boot sector, the
original Star Trail game, and the pinned minimal FreeDOS(98)/FreeCOM disk running
PROBE.COM. The documented compatibility boundaries in [hardware.md](hardware.md)
remain relevant: this is not general PC-98 game compatibility or cycle accuracy.

The following checks were actually executed in the assigned macOS arm64 worktree
on 2026-10-21. Native Rust was 1.94.0; the rustup compiler with the installed Wasm
standard library was 1.99.0. Node was 24.12.0, Python 3.13.13, NASM 3.02, Playwright
1.56.1 and its Chromium 141 build. The build helper explicitly selects rustup's
compiler to avoid mixing a Homebrew Rust compiler with another toolchain's target
standard library.

| Command | Result and evidence |
| --- | --- |
| `python3 tools/prepare_freedos.py` | Passed; checksum-pinned guest releases and both complete source archives downloaded, minimal FAT12 image regenerated with provenance/license files |
| `python3 tools/build_media.py --check` | Passed; firmware/game assembly reproduce checked-in artifacts, cleared hello fixture and both static MIT notices match |
| `cargo test --locked` | Passed: 19 core integration tests and 1 required DOS integration test; no ignored acceptance test |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `cargo tree` | Only `pc98-rust`; no Rust dependencies |
| `npm ci` | Passed; four development packages, no runtime emulator dependency |
| `npx playwright install chromium` | Passed; browser installed locally for the automated run |
| `npm test` | Passed: reproducible media check, actual release Wasm build, local static server, 4 Chromium tests |
| `git diff --check` / staged whitespace check | Passed for the final source files; verbatim upstream source/notices retain their whitespace via scoped attributes |

Native tests cover byte arithmetic flags for every ADD/SUB input pair,
representative word arithmetic/carry/overflow, ModR/M addressing, segment and
20-bit wrap, ROM protection, calls/returns, interrupt stack/IRET, STI inhibition,
HLT wake, shifts, V30 decimal/bit behavior, bounded REP, PIC priority/masking/EOI
and cascade, PIT latches/IRQ, keyboard bytes/modifiers, guest-clocked PCM and queue
bounds, graphics planes/pages/palette/GRCG/text/GDC readback, FDC/DMA reads/writes,
seek/errors/write protection, D88 parsing/export and corrupt media handling.

The independent HelloWorld-PC98 test boots through the firmware/FDC, asserts
`Hello world!!` in VRAM and nonempty rendered glyph pixels, instruction retirement,
one controller read, the PIT divisor and the active beep gate. It does not inject
expected text into the framebuffer.

The game test boots its guest loader, injects real PC-98 keys to collect all three
stars, verifies guest-owned score/move/win bytes, checks changed framebuffer data,
verifies sector-write counts, exports the modified media, and boots the export
again. No host game logic updates those guest fields.

The required DOS test boots the pinned real guest to FreeCOM's `A:\>` prompt,
types `probe` through keyboard IRQs, verifies `PC98 DOS PROBE OK`, guest-driven beep,
and shell recovery. DOS itself processes INT 21h and executable loading. The
emulator contains no INT 21h handler. The exact final image, kernel, shell and
source archive hashes are recorded by the preparation script and described in
[licenses.md](licenses.md).

Browser tests run the same Rust library compiled to `wasm32-unknown-unknown`:

1. Boot and win Star Trail through DOM keyboard events, verify actual canvas pixel
   changes and guest state, guest PCM energy scheduled to a running Web Audio
   context, pause stopping retirement, sector writes, an actual downloaded export,
   file-picker reinsertion and reset. Block all network requests after local
   assets load; none is attempted.
2. Boot the independent hello floppy, check expected text, white canvas pixels,
   FDC read count, clocked beep frequency and Web Audio sample energy.
3. Reject a corrupt D88 without replacing mounted media; insert/eject a second
   drive; operate the virtual PC-98 keyboard.
4. Intercept the Wasm request with a missing and then corrupt response. Both fail
   visibly, keep the demo disabled and never expose a working core.

The PR workflow in `.github/workflows/emulator.yml` runs these mandatory commands
before merge. This node did **not** push, open a PR, run GitHub Actions remotely,
post comments or merge. CI execution on Linux remains for that workflow; the same
commands were run locally. No published-site test, proprietary media, physical
machine test, independent AI PR review or subjective human gameplay was performed
or required for this node. No required post-merge verification is introduced.

## Issue #15 horizontal-blank validation (2026-10-21)

The fixed horizontal timing contract is in [hardware.md](hardware.md#device-ports-and-interrupts).
Bit 6 follows the same elapsed 8 MHz guest clock as other devices, with a shared
phase for text and bitmap GDCs. Status reads add no time or state. The supported
VM/Ap approximations do not change FIFO or vertical-sync behavior, scheduling,
rendering, or programmable display-mode support.

The following checks passed in the assigned macOS arm64 worktree:

| Command | Result |
| --- | --- |
| `python3 tools/prepare_freedos.py` | Pinned GPL inputs and matching complete sources/notices verified; minimal image hash unchanged; original HBLANK probe disk built |
| `python3 tools/prepare_dos_apps.py` | Passed; baseline application fixtures unchanged |
| `python3 tools/prepare_cpu_fixture.py` | Passed |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed |
| `cargo test --locked --test gdc` | All 5 new tests passed, including the DOS guest on VM and Ap |
| `cargo test --locked` | All 69 integration tests passed; no ignored tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed after fixing a new test comment's formatting |
| `cargo tree` | Only pc98-rust; no Rust dependencies |
| `npm ci` | Passed; unchanged development dependencies |
| `npx playwright install chromium` | Passed |
| `npm test -- --config <temporary-port-config>` | Passed: reproducible media/font checks, release Wasm build, publication audit, all 28 actual Chromium/Wasm tests (43.3 seconds) |
| `git diff --check` | Passed |

The first plain `npm test` built Wasm and passed the exact ten-asset publication
audit, then stopped because existing listeners occupied ports 8000/8001. The
successful rerun used a temporary config outside the worktree, importing the
repository config and changing only server ports/base URLs to 18000/18001, with
the same project-path server logic. Tests still ran at `/` and `/pc98-rust/`,
with root asset URLs unavailable in the project-path server. Existing listeners
were left running. No permanent configuration change was needed; the standard
README commands remain the reproduction path when default ports are free.

The new native coverage samples every clock across eight horizontal periods per
profile, checks exact period/duty and both status ports, advances time without
reads, repeats reads with no elapsed time, compares chunked elapsed time, checks
FIFO readback and both vertical boundaries over two frames, and verifies reset
and fixed-mode command behavior. The existing suites cover scheduler/REP bounds,
VM boot/input, Ap planar video, packed-pixel PEGC, audio and persistence.

`guest/hblank.asm` is an original MIT DOS program based on the Issue #13 control.
It initializes the supported video mode, observes four set/clear pairs per port,
and prints `TEXT GDC 0060 PASS`, `BITMAP GDC 00A0 PASS`, and
`HBLANK TRANSITION PASS`. Each poll phase is bounded at 65535 reads. Both native
and browser tests execute it via real FreeDOS/FreeCOM, require successful output
and return to the shell within 16 million scheduler slots per profile, and reject
`TIMEOUT`. The four new browser cases cover VM and Ap at each serving path.

Prepared HBLANK disk SHA-256:
`49931254f45f276fd5280d9fd8a9bd82a548535c4707b2abece117c0dee0c7da`.
Release Wasm: 438021 bytes; generated binaries remain ignored and the probe disk
is excluded from publication. No MARKADIA download or optional game playability
recheck was performed. This verifies the independent status wait, not MARKADIA's
opening, audio or keyboard progression. No push, PR, merge, GitHub comment,
live deployment or post-merge verification was performed or required for #15.

### Issue #15 review revision (2026-10-21)

Integrated the changes from main `123116f4e75c1245d116459e55a9c97977f401f6`
without a merge commit. The timer and horizontal-blank controls, their source and
license notices, and both validation records are retained. The independent
minimal, HBLANK and TIMER image hashes remain unchanged.

The failed [acceptance run](https://github.com/takahirox/pc98-rust/actions/runs/39556249870)
was inspected using its retained Playwright trace. All three full HDD exports
spent 30.0–35.6 seconds transferring numeric arrays through Playwright; the last
export crossed the existing 120-second deadline. The browser test now transfers
the same complete exported bytes as base64, decoded to a Buffer before the
existing assertions. Mouse/IRQ, DOS save, reimport, file-content preservation and
invalid-geometry coverage are retained, with the same timeout and no retries.

| Command | Result |
| --- | --- |
| All six `python3 tools/prepare_*` commands from README | Passed; pinned inputs and source/license outputs prepared |
| `python3 tools/build_font.py --check` | Passed |
| `cargo test --locked` | Passed: 77 native integration tests, including all eight timer and five GDC tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` / `npx playwright install chromium` | Passed |
| `npm test` | Media/font, release Wasm and ten-asset publication checks passed; browser server startup failed because default ports were already occupied |
| `npm test -- --config /tmp/issue15-playwright.config.mjs` | Passed: all 32 Chromium/Wasm tests in 19.2 seconds, including both profiles' timer/HBLANK controls at both serving paths; HDD tests completed in 11.1 and 10.2 seconds |
| `git merge-tree --write-tree <revision> 123116f4e75c1245d116459e55a9c97977f401f6` | Passed: no conflicts, without changing branch ancestry |
| `git diff --cached --check` | Passed |

The temporary browser configuration imported the repository configuration and
changed only the server ports/base URLs to 18000/18001, retaining the project-path
server's restriction to `/pc98-rust/`. Existing listeners were left running.
These results are local macOS arm64 evidence. A passing GitHub acceptance run on
the updated PR head remains pending publication: this revision node must not
push or merge, so it cannot publish the new head or trigger its PR workflow.

## Issue #23: Ap BIOS drawing timing (2026-10-21)

Implemented AH=4Ah CH=06h/16h selection with register preservation, bitmap GDC
request state and PRXDUPD bit 3. The [contract](hardware.md#bios-drawing-timing-int-18ah4a)
records reset defaults, unsupported inputs/VM behavior and timing limits.
The manufacturer's databook printed p.91 (PDF index 107) was checked directly;
no emulator implementation or proprietary guest was acquired.

| Command | Result |
| --- | --- |
| All seven `python3 tools/prepare_*` commands in README | Passed; pinned guests and matching complete source/license outputs retained |
| `python3 tools/build_font.py --check` | Passed |
| `cargo test --locked` | Passed: 84 native tests, zero failures/ignored; includes four new drawing-timing tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `cargo build --locked --release --bin headless` | Passed |
| `target/release/headless "$PWD/fixtures/freedos/drawmode.hdm" 16000000 --ap` | Exit 0; exact original control printed `DRAWMODE RETURNED`, `PROBE SHELL RETURN` and reached the DOS prompt |
| `npm ci` / `npx playwright install chromium` | Passed |
| `npm run build` | Passed: reproducible media/font, production Wasm and exact ten-asset publication audit |
| `npm test -- --config=playwright.issue23.config.js` | Passed: 36 Chromium/Wasm tests at root and project path in 42.6 seconds |
| Local documentation link checks / `git diff --check` | Passed |

Plain `npm test` built successfully, then could not start its server because a
default port was occupied. The successful run used a temporary config importing
the repository config and serving the same `web/` directory at ports 18000/18001.
The project server exposed only `/pc98-rust/`; root asset URLs remained absent.
Existing listeners were left running and the temporary config was removed.
No permanent browser configuration or publication assets changed.

Rebuilding the baseline preparation script from HEAD into a temporary directory
confirmed `minimal.hdm`, `timer.hdm` and `hblank.hdm` are byte-identical to the
new preparation's versions. All existing checked-in guest manifests also passed.
The generated drawing controls include their MIT source and license; the full
FreeDOS folder retains GPL sources and notices. New image SHA-256 values:

- `drawmode.hdm`: `3f942fb2b734ff18792c2ea20b77634582ebdd769c8fa38ef953487c10468e53`
- `drawtest.hdm`: `1427038f73cd42abb39de0dec4d7327564338c50e7fed8d8e11b9276ed189ef1`

Native and Wasm execute the same original controls through real FreeDOS, inspect
both mode states and repeated selection, require shell return and verify reset.
Native tests also cover upper registers, unrelated work bits, GDC/FIFO/scan state,
all unsupported CH values and preserved VM rejection. These are bounded BIOS
contract checks; PIPOO was not acquired or run and no playability is claimed.
No push, PR, merge, comment or post-merge verification was performed or required.

## Issue #29: DMA terminal-count auto-init

Local macOS arm64 validation on 2026-10-23 implements only the
[bounded DMA contract](hardware.md#dma-terminal-count-and-auto-initialization).
Base address/count bytes are retained independently of current progress. Each
serviced byte reports a fresh TC event; FDC completion consumes that event even
when auto-init has already reloaded the registers or the status latch is still
set from a prior command. Existing PC-98 bank mapping and all other hardware
inventory categories and required scope are preserved.

The five original native tests in `tests/dma.rs` cover:

- A 64-case matrix: four channels × increment/decrement × auto-init on/off ×
  counts `0`, `1`, `03FFh`, `FFFFh`, each serviced twice. Independently calculated
  address/count expectations check intermediate reads, wrapping, TC pulses,
  mask/status behavior, no bank carry and channel isolation.
- Shared read/write byte-pointer sequencing and individual low/high address/count
  byte writes after partial progress on every channel. Interleaved channels also
  verify independent bases, masks and accumulated clear-on-read TC latches.
- Both profiles, both transfer directions and lengths 1024, 2048, 1301 and 37:
  two same-direction raw FDC commands reuse the buffer without DMA reprogramming.
  Each verifies exact bytes and guards (whole-image comparison for writes),
  termination before EOT, seven normal result bytes, IRQ 13h and command readiness.
  TC status remains latched between commands, including multi-sector transfers.
- The original MIT boot probe on VM and Ap, including exact disk writes and
  unchanged surrounding media.

The separate test-only `fixtures/dma/autoinit.hdm` is generated offline from
[guest/dma_autoinit.asm](../guest/dma_autoinit.asm) by
`python3 tools/prepare_dma_fixture.py`. Its
[checked manifest](../fixtures/dma-manifest.json) records MIT rights, the exact
`nasm -f bin guest/dma_autoinit.asm -o autoinit.bin` command, source/boot/image
SHA-256 values and the two-million-slot test budget. The generated directory
also retains source, manifest and LICENSE. It is ignored build output outside
`web/`; no existing fixture or distributable asset was modified.

After the synthetic IPL loads the boot sector, the probe uses raw DMA/FDC ports
and its own IR11 handler: two reads of differently patterned sectors, then two
writes with distinct payloads, with only one DMA setup per direction. It checks
reloaded current registers, buffer patterns and guards, normal seven-byte result
phases and a new IRQ for each command. All waits are bounded; it prints exactly
`DMA AUTO INIT PASS` or `DMA AUTO INIT FAIL` and halts. Native `Machine` and
production-Wasm browser tests execute the same image. The four browser cases
cover both VM/Ap profiles at `/` and `/pc98-rust/`, checking the full exported
image, final buffer and guards. This probe needs no DOS or proprietary software.

Regression sensitivity was checked in a temporary export of baseline
`e75c1b848c955111f641cc7ed7611d588faa85fc`, without changing this worktree's
implementation. The new test's API-compatible raw FDC helpers and boot-probe
tests were copied to `tests/dma_regression.rs`, together with the generated disk.
`cargo test --locked --test dma_regression` exited 101 with both tests failing:
readback was `2400h` instead of `2000h`, and the guest printed
`DMA AUTO INIT FAIL`. The unchanged new tests pass with this implementation.

| Command | Result |
| --- | --- |
| `python3 tools/prepare_dma_fixture.py` | Passed; image SHA-256 `6a6ad8aa20488741613e0760924a2e4001d418a16f5a08f53bdacfc4113b3a70` |
| `python3 tools/prepare_freedos.py` | Passed; existing legal guest preparation unchanged |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed after obtaining the exact pinned archive from the Princeton FreeDOS mirror |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed after obtaining the exact pinned archive from the GWDG FreeDOS mirror |
| `python3 tools/build_media.py --check` | Passed; established firmware/demo disk bytes unchanged |
| `python3 tools/build_font.py --check` | Passed; established font bytes unchanged |
| `cargo test --locked` | Passed: 105 native integration tests; zero failures or ignored tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed; zero reported vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font checks, production release Wasm and exact ten-asset publication audit passed; browser startup failed because port 8000 was already occupied |
| `npm test -- --config=playwright.issue29.config.js` | Passed: 48 Chromium/Wasm tests in 24.2 seconds, including all four new DMA cases and all existing acceptance assertions |
| `git diff --check` | Passed |

The CPULEVEL and DEBUG preparation URLs initially returned HTTP 503. The first
native suite therefore stopped on the missing CPU fixture; no tests were skipped.
The successful mirror downloads were
[CPULEVEL](https://mirror.math.princeton.edu/pub/freeDOS/files/util/system/cpulevel/cpulevel2007.zip)
and [DEBUG 1.25](https://ftp.gwdg.de/pub/misc/freedos/files/dos/debug/DEBUG125.zip).
Both matched the existing pinned archive hashes exactly, and their unchanged
preparation scripts then verified their existing manifests. No checksum,
provenance, certificate requirement or existing test assertion was weakened.

The temporary browser config imported the repository config and changed only
ports/base URLs to 18000/18001 and the corresponding server commands. It served
the same `web/` at the root and through a temporary `pc98-rust` symlink, preserving
the project server's rejection of root asset URLs. The pre-existing port-8000
listener was left running; the temporary config was removed after validation.

DMA remains partial: external EOP, transfer/request arbitration, software
requests, verify, memory-to-memory, cascade/rotating priority and broader device
interactions are still excluded or unverified. Broader FDC commands and physical
timing are unchanged. These results establish this contract, not full hardware
accuracy or commercial compatibility. No push, PR, merge or comment was made;
no post-merge verification is required by the Issue.


## Issue #31: DMA master-clear retention

Local macOS arm64 validation on 2026-10-23 covers the
[bounded software master-clear contract](hardware.md#dma-terminal-count-and-auto-initialization).
Any byte written to `001Bh` clears status and the shared byte pointer, sets all
four masks and leaves the modeled controller idle. Current/base address/count
and modes survive; master clear neither zeroes programming nor reloads bases.
Construction/reset and the existing policy of zeroing external PC-98 banks are
unchanged. The bank policy is preserved behavior, not verified board wiring.
Command/Request/Temporary state remains unmodeled.

Three new native regressions extend the existing five DMA tests:

- All four channels × increment/decrement × auto-init on/off × both initial
  byte-pointer phases × data `00h`, `55h`, `FFh`. All TC bits are latched first,
  every current address/count differs from its base, and masks start mixed.
  Repeated clears check immediate low/high readback without an intervening `19h`,
  cleared status, all masks and retained modes. Subsequent service after unmasking
  checks intermediate progress and TC; auto-init reload proves saved bases survive.
- Low/high partial programming of address/count on each channel after service,
  followed by master clear, current readback and eventual base reload.
- VM and Ap raw-port FDC READ/WRITE DATA in bank zero: master clear, a masked
  command with exact error result `40 10 00 00 00 02 03`, then unmasking alone and
  successful transfer with exact normal results. Each command checks its new
  completion IRQ, seven result bytes and readiness. Masked commands leave the
  full buffer/guards and disk unchanged and do not increment transfer counters;
  successful commands check exact data, guards and the complete disk image.

The existing original MIT [boot probe](../guest/dma_autoinit.asm) retains its
consecutive auto-init reads/writes, sticky-status, reload, guard, result and IRQ
checks. The original final `B6h` buffer assertion now runs inside the guest before
the master-clear phase reuses that buffer. The added phase programs DMA once per
direction, enters master clear with the pointer high, immediately reads retained
address/count and checks cleared status, issues a masked command, then unmasks
without rewriting address/count/mode. A retained read verifies every payload byte;
a retained write changes sector 8 to `4Dh`, while the masked write's sector 7 and
all other media guards stay unchanged. Host tests compare the entire disk and
final buffer. IRQ/result waits remain bounded by `FFFFh` iterations each. Success
prints `DMA AUTO INIT PASS / DMA MASTER CLEAR PASS`; failures identify either
`DMA AUTO INIT FAIL` or `DMA MASTER CLEAR FAIL`.

Build with `python3 tools/prepare_dma_fixture.py`; the exact assembly command is
`nasm -f bin guest/dma_autoinit.asm -o autoinit.bin`. Only the affected
[DMA fixture manifest](../fixtures/dma-manifest.json) was intentionally regenerated
(`--update-manifest`); all other fixture manifests and publication assets are
unchanged. The generated image, source, manifest and MIT LICENSE stay together in
the ignored `fixtures/dma/` directory, outside `web/`. No commercial ROM or guest
software is required. Final SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| `guest/dma_autoinit.asm` | `38c31d31bc8770c6f0b0e977881c8a59c786715f8db4e6244e653799cef4e322` |
| Assembled 1024-byte boot sector | `ee46f6fb831b86d93d7f1b6b0f7ccfac2da0f5f38d80d3f68e0800c752590d78` |
| `fixtures/dma/autoinit.hdm` | `f1d2ab1af43621615cf0af8636b72bd7069441bac870d246004538e2fe98258c` |

Native `Machine` and production release Wasm execute that same image on both VM
and Ap, with a budget of 20 calls of 100,000 instruction slots (two million total)
per execution. All four browser cases cover both profiles at `/` and
`/pc98-rust/`. Existing auto-init and stale-status regressions remain enabled.

Regression sensitivity was checked in an isolated `git archive` export of
baseline `beb4c7d3a5286738a2c29c11855773881e70dc14` under
`target/issue31-baseline`, copying the final `tests/dma.rs` and generated image.
The following commands each exited 101 there, while they pass with the fix:

```sh
cargo test --locked --test dma master_clear_retains_programming_matrix -- --exact
cargo test --locked --test dma original_boot_probe_runs_on_both_profiles -- --exact
```

The matrix failed on the very first retained byte: `00h` instead of `41h`.
The guest passed its preceding auto-init checks and printed
`DMA MASTER CLEAR FAIL` in the focused phase. Baseline implementation files were
not changed; both runs used the final source-built probe image above.

| Command | Result |
| --- | --- |
| `python3 tools/prepare_dma_fixture.py` | Passed; final manifest and image verified |
| `python3 tools/prepare_freedos.py` | Passed; pinned binaries and complete source archives retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed after pinned mirror download described below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed after pinned mirror download described below |
| `cargo test --locked --test dma` | Passed: 8 tests, zero failures or ignored tests |
| `cargo test --locked` | Passed: 108 native integration tests, zero failures or ignored tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; established firmware and demo disks unchanged |
| `python3 tools/build_font.py --check` | Passed; existing font unchanged |
| `npm ci` | Passed; zero reported vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font, production Wasm and ten-asset publication checks passed; browser server startup failed because port 8000 was occupied |
| `npm test -- --config=playwright.issue31.config.js` | Passed: all 48 Chromium/Wasm tests in 26.2 seconds, including the four extended DMA cases |
| `git diff --check` | Passed |

CPULEVEL and DEBUG preparation initially failed with HTTP 503 from ibiblio; no
fixtures or tests were skipped. The exact pinned archives were downloaded from
[Princeton CPULEVEL mirror](https://mirror.math.princeton.edu/pub/freeDOS/files/util/system/cpulevel/cpulevel2007.zip)
and [GWDG DEBUG mirror](https://ftp.gwdg.de/pub/misc/freedos/files/dos/debug/DEBUG125.zip),
verified against the unchanged script hashes before writing into
`fixtures/dos-apps/`, then their original preparation commands passed. No
checksum, license/source requirement or test assertion was weakened.

The temporary browser config imported `playwright.config.js`, preserving all
projects, tests, workers and assertions; it changed only the two base URLs and
server commands to ports 18000/18001. The root server command was
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`. The project server
was a temporary copy of `tests/browser/serve_project.py` with `WEB` set to this
worktree's `web/` and port 8001 changed to 18001; its temporary `pc98-rust` symlink
and rejection of root asset URLs were preserved. The pre-existing port-8000
listener was left alone. The temporary config was removed after validation.

DMA remains **partial**: software requests, command controls, verify transfers,
demand/block/single arbitration, external EOP, memory-to-memory, cascade/rotating
priority, bank-reset fidelity and broader device interactions are incomplete or
unverified. Limited probe success establishes neither full hardware accuracy nor
commercial compatibility. No other inventory category changes scope or gains a
verification claim. No post-merge verification is required by Issue #31.

## Issue #33: DMA controller disable

Implemented only 8237 command bit 2 at PC-98 write port `11h`, on VM and Ap.
The existing FDC consumer samples controller enable and channel mask together at
scheduled service time. An unserviced valid command completes through the existing
masked-channel overrun path. Command writes retain programming, bases, modes,
banks, masks, TC and the shared byte pointer. Machine reset and software master
clear clear command state; master clear still retains address/count/modes and
masks every channel. Other command controls remain unsupported. See the
[bounded contract](hardware.md#dma-terminal-count-and-auto-initialization).

Three added native tests extend the existing eight DMA tests:

- All four channels, both pointer phases and address/count byte programming while
  disabled; repeated disable/enable writes, status reads while disabled, unrelated
  programming/masks/latched TC retention and subsequent auto-init base reload.
- Raw READ/WRITE DATA on VM and Ap, auto-init on/off, disabled before submission
  or after 159 of the 160 scheduled clocks. Unmasked disabled commands return
  exactly `40 10 00 00 00 02 03`, with IRQ, seven bytes and readiness. Buffer and
  guards, complete disk, all current registers/modes/banks/masks, unrelated TC,
  disk write counter and FDC counters remain unchanged. Enabling does not replay
  an error or bypass a channel mask. Clearing disable alone then permits a fresh
  unmasked command with retained programming, exact payload, normal IRQ/result
  and the correct TC, mask and auto-init behavior.
- Machine reset and master clear clear disable on both profiles, without bypassing
  masks; unmasking permits retained master-clear programming to transfer.

The original MIT `guest/dma_autoinit.asm` remains a 1024-byte source-built boot
sector. All earlier auto-init and master-clear assertions remain; the earlier
final buffer check now runs in the guest before buffer reuse. The added phase
checks repeated disable/enable writes, high-pointer and sticky-status retention,
blocked unmasked read/write and successful fresh commands after enabling. A
successful write adds `D2h` to sector 4; existing `69h`, `B6h` and `4Dh` write
assertions at sectors 5, 6 and 8 remain, and sector 7 remains an untouched guard.
Native tests check the complete resulting disk and exact counters (five reads,
including IPL, and four writes). Browser tests compare the complete exported disk,
final buffer and guards. No game assertion was changed.

Success prints `DMA AUTO INIT PASS / DMA MASTER CLEAR PASS / DMA DISABLE PASS`;
failures retain phase-specific FAIL markers. Result/IRQ polls remain bounded at
`FFFFh` iterations, and native and production-Wasm runs retain a two-million-slot
budget per profile/path. The exact same image passed on native VM/Ap and production
Wasm VM/Ap at `/` and `/pc98-rust/`. It requires no DOS, commercial ROM, proprietary
firmware or newly acquired game. The MIT source, copied license and generated
image stay outside `web/`.

The [manifest](../fixtures/dma-manifest.json), regenerated with
`python3 tools/prepare_dma_fixture.py --update-manifest`, records:

| Item | Value |
| --- | --- |
| Assembly | `nasm -f bin guest/dma_autoinit.asm -o autoinit.bin` |
| License | MIT; repository `LICENSE` copied beside the image |
| Source SHA-256 | `1119cbdcd8b32f1b3661eb4854a8e988041737866469f1bc50ac790f4afa1adc` |
| Boot SHA-256 | `196d87f97f8be853700b90abde49ea3bc72d25423d890ff690c77d91cd5fdb83` |
| Image SHA-256 | `464524cd5e8c7701a7a8cd264ee70c767c51ed269dc89979e4478e1920158fef` |

Baseline-negative validation used an isolated temporary directory populated by
`git archive 131086f8264026569d25f07c8e83e70ab02860ca`. Only the final
`tests/dma.rs` and final generated `fixtures/dma/autoinit.hdm` were copied into
that directory. Both baseline implementation files (`src/devices.rs`,
`src/bus.rs`) were byte-compared with `git show` afterward and remained unchanged.
From that isolated directory:

| Command | Expected negative result |
| --- | --- |
| `cargo test --locked --test dma raw_fdc_controller_disable_samples_service_and_retains_programming -- --exact --nocapture` | Exit 101: disabled command incorrectly returns successful `00 00 00 00 00 02 03` instead of `40 10 00 00 00 02 03` |
| `cargo test --locked --test dma original_boot_probe_runs_on_both_profiles -- --exact --nocapture` | Exit 101: guest prints `DMA DISABLE FAIL` in the new phase |

The same tests and image pass with the implementation. No baseline source was
substituted into this implementation worktree.

Validation ran on Darwin 25.6.0 arm64, with Homebrew Rust/Cargo 1.94.0 for native
checks, rustup Rust 1.99.0 and `wasm32-unknown-unknown` for the production Wasm
build (the existing `build_web.py` compiler selection), NASM 3.02, Python 3.13.13,
Node 24.12.0, npm 11.6.2 and pinned Playwright 1.56.1 Chromium.

| Command | Result |
| --- | --- |
| `python3 tools/prepare_dma_fixture.py` | Passed; final source/image/manifest verified |
| `python3 tools/prepare_freedos.py` | Passed; pinned binaries and complete matching source archives retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed after pinned mirror download below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed after pinned mirror download below |
| `cargo test --locked --test dma` | Passed: 11 tests, zero failed or ignored |
| `cargo test --locked` | Passed: 111 native integration tests, zero failed or ignored; existing hardware, DOS/application/game coverage retained |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; established firmware/demo images unchanged |
| `python3 tools/build_font.py --check` | Passed; established font unchanged |
| `npm ci` | Passed |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font, release Wasm and ten-asset site checks passed; browser server startup failed because port 8000 was occupied |
| `npm test -- --config=playwright.issue33.config.js` | Passed: all 48 Chromium/Wasm tests in 24.5 seconds, including the four extended DMA cases and all existing game regressions |
| `git diff --check` | Passed |

CPULEVEL and DEBUG preparation initially received HTTP 503 from ibiblio. The
following checksum-verifying mirror fetches populated only the ignored fixture
cache, then the unchanged preparation commands above passed. No fixture,
assertion, provenance requirement or checksum was skipped or weakened:

```sh
python3 - <<'PY'
import sys
sys.path.insert(0, 'tools')
from prepare_dos_apps import fetch
from prepare_cpu_fixture import ARCHIVE_SHA
fetch('cpulevel2007.zip', 'https://mirror.math.princeton.edu/pub/freeDOS/files/util/system/cpulevel/cpulevel2007.zip', ARCHIVE_SHA)
PY
python3 - <<'PY'
import sys
sys.path.insert(0, 'tools')
from prepare_dos_apps import fetch
from prepare_device_fixture import HASH
fetch('debug125.zip', 'https://ftp.gwdg.de/pub/misc/freedos/files/dos/debug/DEBUG125.zip', HASH)
PY
```

The temporary browser config imported `playwright.config.js`, retained its test
selection, projects, workers and assertions, and changed only server commands and
URLs to free ports 18000/18001. The root command was
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`; the project command
was `python3 /tmp/issue33-serve-project.py`. That temporary server copied
`tests/browser/serve_project.py`, setting `WEB` to this worktree's `web/` and port
to 18001, preserving the project symlink and rejection of root asset URLs. The
pre-existing port-8000 listener was left alone. Temporary configuration was
removed after validation.

DMA and integration remain **partial**. Other command controls, software requests,
verify transfers, demand/block/single service and arbitration, rotating priority
and cascade, external EOP, memory-to-memory transfers, bank-reset fidelity and
broader device interactions remain unsupported or unverified. This adds no
per-byte FDC scheduling or physical timeout/DRQ/DACK/media claim. Limited probes
establish neither full hardware accuracy nor commercial compatibility. Other
inventory statuses and required scopes, including protected mode, paging and
basic FPU work, remain unchanged. No post-merge verification is required by #33.

## Issue #35: Ap real-mode GDTR (2026-10-23)

Implemented only LGDT/SGDT in the Ap decoder, with per-instance GDTR and the
[documented reset policy, specification distinction and unsupported boundaries](hardware.md#ap-real-mode-gdtr).
The instructions use the existing Bus and real-mode fault mechanism. The VM
V30 decoder is unchanged. No existing test assertion was removed or weakened.

The eight original `gdtr_` tests in `tests/cpu386.rs` cover the four operand/address
size combinations, zero/maximal limits/bases, high-byte retention and clearing,
SIB and default SS, all segment overrides including FS/GS, repeated overrides,
unaligned operands, exact six-byte stores/guards, preserved registers/flags,
independent CPUs and Machine reset. Boundary cases cross both operand sizes,
both address sizes and DS/SS for loads and stores: `FFFAh` succeeds; `FFFBh`,
`10000h` and `FFFFFFFFh` fault before partial effects. Initialized IVT/stack tests
assert vector, saved first-prefix IP, FLAGS, unchanged GDTR/destination bytes,
IRET/retry after address repair, repeated invalid-register/LOCK faults, A20 reads
and writes, retained linear-base bits, and ROM protection.

The original MIT `guest/gdtr.asm` assembles offline into a single boot sector,
zero-padded to a 77×2×8×1024 floppy. Its handlers validate vector routing, saved
CS and first-prefix IP, count faults with explicit bounds, repair DS/SS addresses
and IRET to retry. Register/LOCK #UD handlers validate and advance past their
invalid instructions. The guest checks exact readback, high-byte clearing,
reset state, last legal operand and guards before emitting `GDTR PASS` or
`GDTR FAIL`. The fixture includes source, license and manifest outside `web/`.

| Fixture provenance | Value |
| --- | --- |
| Assembly | `nasm -f bin guest/gdtr.asm -o gdtr.bin` |
| License | Original MIT; repository `LICENSE` copied beside image |
| Source SHA-256 | `ceb4c144302af92c164f6eddae475b43c839bb3c6a8e8f76956711ff182d2f3e` |
| Boot SHA-256 | `d4ed5c90c5d05495ac6ac82818ca42f8faabe3f43907518cf375f2c13dd9a8e5` |
| Image SHA-256 | `69fccf73900116e3fe7143c9f275676c1b19ccdfc4c94d2c70f73021259758f2` |
| Execution budget | At most 2,000,000 instruction slots per run, in 100,000-slot slices |

The native boot test and actual production release-Wasm browser test execute
that identical image on Ap, before and after reset. Both require PASS, absence
of FAIL/errors, the entire exact 80-byte result/guard region at `8000h`, the
seven bytes at `FFF9h–FFFFh`, and an unchanged disk. Browser execution covers
both `/` and `/pc98-rust/`. Existing CPULEVEL, hardware, DOS/application, VM,
DMA and game assertions remain in the full suites.

Baseline-negative evidence used `git archive
1c9dcf67d23827f7ed62609affdea6f9c33f5c10` in the isolated ignored directory
`target/gdtr-baseline`. Only the final probe image and a test file containing the
existing CPU helpers plus these two final tests were added:

```sh
mkdir -p target/gdtr-baseline
git archive 1c9dcf67d23827f7ed62609affdea6f9c33f5c10 | tar -x -C target/gdtr-baseline
python3 - <<'PY'
from pathlib import Path
import shutil
source = Path('tests/cpu386.rs').read_text()
helpers = source[:source.index('#[test]')]
focused = source[source.index('#[test]\nfn gdtr_full_base_'):source.index('#[test]\nfn gdtr_sib_')]
probe = source[source.index('#[test]\nfn gdtr_original_boot_probe_'):]
root = Path('target/gdtr-baseline')
(root / 'tests/gdtr_negative.rs').write_text(helpers + focused + probe)
(root / 'fixtures/gdtr').mkdir(parents=True, exist_ok=True)
shutil.copyfile('fixtures/gdtr/gdtr.hdm', root / 'fixtures/gdtr/gdtr.hdm')
PY
cargo test --locked --manifest-path target/gdtr-baseline/Cargo.toml --test gdtr_negative
```

The baseline compiled, then exited 101: both
`gdtr_full_base_survives_both_stores_then_word_load_clears_high_byte` and
`gdtr_original_boot_probe_before_and_after_machine_reset` failed with
`unimplemented i486 opcode 0F 01`. Baseline `src/cpu.rs`, `src/bus.rs` and
`src/lib.rs` were byte-compared against `git show` and remained unchanged.
The same tests and image pass with the implementation.

Validation ran on Darwin 25.6.0 arm64, Homebrew Rust/Cargo 1.94.0 natively,
rustup Rust 1.99.0 with `wasm32-unknown-unknown` for production Wasm (the existing
`build_web.py` compiler selection), NASM 3.02, Python 3.13.13, Node 24.12.0,
npm 11.6.2 and pinned Playwright 1.56.1 Chromium.

| Exact command | Outcome |
| --- | --- |
| `python3 tools/prepare_gdtr_fixture.py --update-manifest` | Created the audited original fixture manifest |
| `python3 tools/prepare_gdtr_fixture.py` | Passed; source, boot/image hashes and manifest reproduced offline |
| `python3 tools/prepare_dma_fixture.py` | Passed; existing fixture/hash unchanged |
| `python3 tools/prepare_freedos.py` | Passed; pinned binaries and complete matching source archives retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed after checksum-pinned mirror fetch below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed after checksum-pinned mirror fetch below |
| `cargo test --locked --test cpu386 gdtr_` | Passed: 8 tests, zero failed/ignored |
| `cargo test --locked` | Passed: 119 native integration tests, zero failed/ignored; existing regression assertions retained |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; established firmware/demo images unchanged |
| `python3 tools/build_font.py --check` | Passed; established font unchanged |
| `npm ci` | Passed |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font, release Wasm and site checks passed; browser server startup failed because port 8000 was occupied |
| `npx playwright test --config=.gitweave-playwright.config.js tests/browser/gdtr.spec.js` | Passed: 2 cases, before/after reset at both paths (5.7 seconds) |
| `npm test -- --config=.gitweave-playwright.config.js` | Passed: release Wasm (446455 bytes), ten-asset publication audit, all 50 Chromium/Wasm tests (32.8 seconds), zero skipped |
| `git diff --check` and local documentation-link checks | Passed |

CPULEVEL and DEBUG initially returned HTTP 503 at their pinned ibiblio URLs.
CPULEVEL also failed with an alternate query/hostname; the FTP hostname refused
the connection. These exact mirror URLs supplied archives with the original
pinned SHA-256 values. The DEBUG mirror path containing `vojta/1.25` returned
404; its root DEBUG path below succeeded. Only ignored fixture caches were
populated; the unchanged preparation scripts then verified full manifests.
Equivalent checksum-verifying cache preparation is:

```sh
python3 - <<'PY'
import sys
sys.path.insert(0, 'tools')
from prepare_dos_apps import fetch
from prepare_cpu_fixture import ARCHIVE_SHA
from prepare_device_fixture import HASH
fetch('cpulevel2007.zip', 'https://mirror.math.princeton.edu/pub/freeDOS/files/util/system/cpulevel/cpulevel2007.zip', ARCHIVE_SHA)
fetch('debug125.zip', 'https://mirror.math.princeton.edu/pub/freeDOS/files/dos/debug/DEBUG125.zip', HASH)
PY
python3 tools/prepare_cpu_fixture.py
python3 tools/prepare_device_fixture.py
```

The temporary browser configuration imported `playwright.config.js` and changed
only server commands/URLs to free ports 18000/18001. It preserved both projects,
all tests, workers, assertions and budgets. Its root server command was
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`; its project command
was `python3 target/gdtr-serve-project.py`. The latter copied
`tests/browser/serve_project.py`, setting `WEB` to this worktree's `web/` and
port to 18001, retaining the project symlink and rejection of root asset URLs.
The occupied listener was left alone; the temporary config was removed after
validation. No guest assertion, hash or provenance check was skipped.

All 26 inventory categories retain the Issue's supplied status and full scope.
Protected mode remains unimplemented: CR0/PE transitions, descriptor lookup,
caches/validation, CPL/privilege transitions, gates and task-state behavior remain
outside this prerequisite. Paging and basic x87 remain separate work. These
bounded probes establish neither full hardware accuracy nor commercial
compatibility. Issue #35 requires no post-merge verification.
