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

## Issue #37: Ap real-mode IDTR and interrupt-table relocation (2026-10-23)

Implemented per-CPU LIDT/SIDT, architectural reset base 0/limit 03FFh, relocated
real-mode vector lookup, inclusive table limits and bounded shutdown when
exception 8 cannot be delivered. The [hardware boundary and specification
choices](hardware.md#ap-real-mode-idtr) document full-base SIDT stores despite
contradictory compatibility text, and target-era exception 13 for SS-selected
six-byte operands despite the modern SIDT exception table's exception 12.
VM retains its V30 decoder and fixed IVT. Group `/1` and `/3` unsupported
expectations were replaced with positive acceptance; `/4`–`/7` and VM rejection
assertions remain. Existing GDTR, CPULEVEL, hardware, DOS, application, VM and
game assertions are preserved.

The fourteen original `idtr_` native tests cover all four operand/address-size
combinations, unaligned operands, ModR/M/SIB, default SS and every segment
override, repeated prefixes, exact six-byte stores with guards, high-base
retention/clearing, preserved registers/flags, independent CPUs and reset
readback. A matrix covers loads/stores, DS/SS and both operand sizes at FFFAh,
FFFBh, address32 10000h and FFFFFFFFh. It checks atomic #GP, first-prefix IP,
unchanged IDTR/destination, repair/IRET/retry, and restartable register/LOCK #UD.
Operand and vector Bus/A20 mapping, ROM write protection, retained unmapped
base metadata and wrapping 32-bit vector arithmetic are asserted explicitly.

Relocation tests install conflicting address-zero vectors and assert software
INT, INT3, taken/untaken INTO, PIC IRQ, CPU faults, TF traps and public native
injection. They inspect exact six-byte frames, saved CS/IP/FLAGS, restart versus
resume addresses, IRET, AC clearing, IRQ acknowledgement/EOI, STI and MOV/POP SS
shadows. The final complete entry succeeds at C3h; C2h routes the same prefixed
INT to relocated vector 8 with no partial original frame or error-code word.
Limit 23h permits vector 8; 22h enters shutdown. Bounded repeated stepping and
native injection cannot resume it or acknowledge a newly pending maskable IRQ;
Machine reset clears shutdown and restores IDTR. IRQ, fault, trap and native
injection table-limit failures are also checked without host errors.

The original MIT `guest/idtr.asm` assembles offline into one 1024-byte boot
sector, zero-padded to a 77×2×8×1024 floppy. Source, repository MIT license and
manifest are copied beside the ignored image, outside `web/`. The guest checks
reset readback, 24/32-bit bases, full stores, address32, relocated software
entries, fault repair/retry, invalid forms, inclusive limit #DF and wrapping
linear addresses. It validates saved first-prefix IP/CS and the #DF FLAGS slot
without an error-code word. Fixed-IVT entries conflict with the relocated
handlers. It emits `IDTR PASS` or `IDTR FAIL`, then the successful path enters
terminal shutdown with limit 22h. The host requires shutdown separately from
the PASS text, stable retirement through another 100,000 slots, exact complete
80-byte results/guards at 8000h and seven bytes at FFF9h, no error, and unchanged
disk bytes. Native checks additionally assert HLT is false and pending IRQ
acknowledgement stays clear. The identical probe runs before and after Machine
reset natively and in actual production release Wasm at both URL paths.

| Fixture provenance | Value |
| --- | --- |
| Assembly | `nasm -f bin guest/idtr.asm -o idtr.bin` |
| License | Original MIT; repository `LICENSE` copied beside image |
| Source SHA-256 | `b744ec58fad1d496fbd1ca4c3acefa6a40b735251454027e56fb01822a709752` |
| Boot SHA-256 | `33344d1a742b91cfabbf2662b8b1a22545a0c5fd20cb4a428d538983edffd730` |
| Image SHA-256 | `ed28cac29c193143f017f6d1ad4168e6a7aa2515a4515777a0911fe7040344e5` |
| Exact result bytes and guards | [fixtures/idtr-manifest.json](../fixtures/idtr-manifest.json), `result_hex` and `last_operand_hex` |
| Execution budget | At most 2,000,000 slots per probe, in 100,000-slot slices |

Baseline-negative evidence uses the final unchanged
`idtr_minimal_lidt_sidt_reproduction` test. It failed on the initial worktree
with `unimplemented i486 opcode 0F 01 /3`, then passed after implementation.
It was also run against an isolated archive of the Issue's exact baseline:

```sh
mkdir -p target/idtr-baseline
git archive 27e5655302458cd09cac6eb7f6f54065a811b20f | tar -x -C target/idtr-baseline
python3 - <<'PY'
from pathlib import Path
s = Path('tests/cpu386.rs').read_text()
helpers = s[:s.index('#[test]')]
start = s.index('#[test]\nfn idtr_minimal_lidt_sidt_reproduction')
end = s.index('// Original Issue #37', start)
Path('target/idtr-baseline/tests/idtr_negative.rs').write_text(helpers + s[start:end])
PY
cargo test --locked --manifest-path target/idtr-baseline/Cargo.toml --test idtr_negative
```

The baseline compiled and exited 101 with that same unsupported `/3` error.
Its `src/cpu.rs`, `src/bus.rs` and `src/lib.rs` were byte-compared with `git show`
and remained identical to 27e5655. The isolated helper copy produced only unused
`Machine` import/`fault` helper warnings. No implementation was substituted into
the baseline. The unchanged test passes in the final fourteen-test focused run.

Validation environment: Darwin 25.6.0 arm64; Homebrew Cargo/Rust 1.94.0 for
native tests; rustup Rust 1.99.0 and `wasm32-unknown-unknown` for the existing
production `build_web.py` compiler selection; NASM 3.02, Python 3.13.13,
Node 24.12.0, npm 11.6.2, pinned Playwright 1.56.1 Chromium.

| Exact command | Outcome |
| --- | --- |
| `python3 tools/prepare_idtr_fixture.py --update-manifest` | Created the original audited manifest |
| `python3 tools/prepare_idtr_fixture.py` | Passed; source/boot/image hashes and result contract reproduced offline |
| `python3 tools/prepare_dma_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_gdtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_freedos.py` | Passed; pinned guest binaries and complete matching sources retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed; independent applications and source archives prepared |
| `python3 tools/prepare_cpu_fixture.py` | Initial attempt and retry failed HTTP 503; passed after checksum-verified mirror cache preparation below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed; pinned guest sources and source-built assembler retained |
| `python3 tools/prepare_device_fixture.py` | Initially failed HTTP 503; passed after checksum-verified mirror cache preparation below |
| `cargo test --locked --test cpu386 idtr_` | Passed: 14 tests, zero skipped |
| `cargo test --locked` | Passed: all 133 native integration tests, zero skipped; unit/doc harnesses also passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; firmware and media hashes unchanged |
| `python3 tools/build_font.py --check` | Passed; CG hash unchanged |
| `npm ci` | Passed; locked dependencies installed |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font checks, release Wasm and ten-asset site audit passed; browser startup failed because port 8000 was occupied |
| `npx playwright test --config=.gitweave-playwright.config.js tests/browser/idtr.spec.js` | Passed: both projects, before/after reset (2 cases, 5.8 seconds) |
| `npm test -- --config=.gitweave-playwright.config.js` | Passed: production release Wasm (447263 bytes), ten-asset audit and all 52 browser tests in 25.2 seconds; zero skipped |
| `git diff --check` and local documentation target/anchor checks | Passed |

CPULEVEL and DEBUG's pinned ibiblio URLs returned HTTP 503. As in the previous
validation, the following exact mirror archives were verified with the original
pinned SHA-256 values before writing only ignored fixture caches. Unchanged
preparation scripts then verified their complete manifests; no hash, license,
source requirement or assertion was weakened.

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

The temporary `.gitweave-playwright.config.js` imported the repository config,
changing only both base URLs/server commands to free ports 18000/18001. It kept
both projects, all tests, workers, assertions and budgets. The root command was
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`. The project
command was `python3 target/idtr-serve-project.py`, a copy of
`tests/browser/serve_project.py` setting `WEB` to this worktree's `web/` and port
to 18001. Its temporary project symlink and root-asset rejection were preserved.
The pre-existing port-8000 listener was left alone. The temporary config was
removed after validation.

Local review checked the complete diff against Issue #37, including operand
atomicity, saved-IP distinctions, all delivery callers, shutdown bounds/reset,
32-bit native/Wasm arithmetic, fixture provenance, CI preparation and preserved
regressions. No blocking local review findings remain. All 26 inventory
categories retain their supplied statuses and full scopes: protected mode and
CPU386 remain partial; paging and x87 remain unimplemented. CR0/PE transitions,
protected gates, descriptors/caches, privilege changes, LDTR/TR, task switching,
NMI and general nested-fault/invalid-stack behavior remain outside this change.
Probe success does not establish full protected-mode accuracy or commercial
compatibility. No PR/CI-success or merge is claimed; Issue #37 requires no
post-merge verification.

## Issue #39: Ap real-mode SMSW readback (2026-10-23)

Implemented `0F 01 /4` from a CPU-local low-word status representation, reset to
0010h by construction and Machine reset. Memory stores preflight exactly two
bytes and use the Bus; register stores preserve upper halves with either operand
size. The operand32 upper-half choice and reserved low-word zeros are deterministic
emulator policies. Intel486 PRM §10.1 Figure 10-2 supplies the reset bit choices;
its SMSW real-address table (printed p. 26-268) supplies exception 13 even for SS
operands. The modern SDM instead lists exception 12 for SS overruns. The tables
disagree; [the hardware contract](hardware.md#ap-real-mode-smsw) documents this
choice and the SDM operand-width/LOCK rules. General memory faults are unchanged.

Seven original `smsw_` tests cover all eight registers with nonzero upper halves,
all operand/address-size combinations, repeated overrides, ModR/M/SIB, default SS,
all segment overrides, unaligned stores, exact width/guards, flags and unrelated
state. Boundary cases cover FFFE, FFFF, address32 10000 and FFFFFFFF for DS and SS.
Fault checks compare all RAM against the original bytes plus only the six-byte
frame, verify first-prefix IP/CS/FLAGS, then repair the effective address and
IRET/retry. LOCK register/memory cases also fault again when unrepaired, then
succeed after prefix repair. Bus/A20, ROM write protection, CPU isolation and
reset readback have direct tests. The full CPU386 run preserves GDTR/IDTR and
general #SS acceptance, plus VM rejection of LGDT and both SMSW forms.

The original MIT [boot probe](../guest/smsw.asm) is assembled offline by
`tools/prepare_smsw_fixture.py`; it uses no downloaded guest code, DOS or
proprietary ROM. [The checked-in manifest](../fixtures/smsw-manifest.json) records
the NASM command, source/boot/image hashes, expected 80-byte result region, final
boundary operand, guards, license and execution budget. Image SHA-256:
`f8cfb119774c110bb947b9dd5fa06ac2d2feb9e5067f07df038a00cddd4bbd1a`.
The same source-built image runs natively and in production release Wasm at `/`
and `/pc98-rust/`, before and after Machine reset. Each boot requires explicit
`SMSW PASS`, no `FAIL` or host error, exact result/guard bytes, unchanged disk
bytes and at most 2,000,000 scheduler slots. The guest handles DS/SS #GP and LOCK
register/memory #UD, checking six-byte first-prefix frames and repairing/retrying.
CI prepares this fixture; generated image/source/license copies remain ignored
under `fixtures/smsw/`, outside `web/`.

Baseline evidence: before editing production code, the original
`smsw_minimal_register_reproduction` failed with
`unimplemented i486 opcode 0F 01 /4`. The upstream worktree had no differences
from `dbd65302d2dea6fe6cc22d5dcd356bd3a292b2df`. For an independent check, Python
`tarfile` extracted `git archive dbd65302d2dea6fe6cc22d5dcd356bd3a292b2df` into
ignored `target/smsw-baseline/`. The unchanged `cpu()` helper and exact new minimal
test were copied to `tests/smsw_baseline.rs` there, with only the needed imports.
The baseline's `src/cpu.rs`, `src/bus.rs` and `src/lib.rs` were byte-compared
against `git show dbd65302:<path>`. No implementation was substituted and no
assertion was weakened. This exact command exited 101 with the same `/4` error:

```sh
cargo test --locked --manifest-path target/smsw-baseline/Cargo.toml --test smsw_baseline smsw_minimal_register_reproduction
```

The unchanged test passes on the implementation. It executes `0F 01 E0`, starting
with EAX=ABCDFFFFh and nontrivial flags, requires EAX=ABCD0010h and unchanged flags,
and executes the following NOP to verify continued execution.

Environment: Darwin 25.6.0 arm64; Homebrew native Rust/Cargo 1.94.0;
rustup Rust 1.99.0 with `wasm32-unknown-unknown`, selected by the repository's
production `build_web.py`; NASM 3.02; Python 3.13.13; Node 24.12.0; npm 11.6.2;
locked Playwright 1.56.1 Chromium.

| Exact command | Outcome |
| --- | --- |
| `python3 tools/prepare_smsw_fixture.py --update-manifest` | Created the original fixture manifest |
| `python3 tools/prepare_smsw_fixture.py` | Passed; offline source/boot/image and result contract reproduced |
| `python3 tools/prepare_dma_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_gdtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_idtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_freedos.py` | Passed; pinned binaries and complete matching GPL source archives retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed; original manifest and licensed applications/sources retained |
| `python3 tools/prepare_cpu_fixture.py` | Initial URL returned HTTP 503; passed after checksum-verified mirror cache preparation below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed; pinned sources and source-built guest assembler retained |
| `python3 tools/prepare_device_fixture.py` | Passed using checksum-verified mirror cache below |
| `cargo test --locked --test cpu386 smsw_` | Passed: 7 tests, zero skipped |
| `cargo test --locked --test cpu386` | Passed: 50 tests, zero skipped |
| `cargo test --locked` | Passed: all 140 native integration tests, zero skipped; unit/doc harnesses also passed |
| `cargo clippy --all-targets -- -D warnings` | Passed after factoring a local test tuple type alias |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; firmware/media hashes unchanged |
| `python3 tools/build_font.py --check` | Passed; CG hash unchanged |
| `npm ci` | Passed; locked dependencies installed |
| `npx playwright install chromium` | Passed |
| `npm test` | Media/font checks, release Wasm and ten-asset site audit passed; server startup failed because port 8000 was already occupied |
| `npm test -- --config=.gitweave-playwright.config.js` | Passed: release Wasm (447626 bytes), ten-asset site audit and all 54 browser tests in 26.5 seconds, zero skipped |
| `git diff --check` | Passed |

The CPULEVEL fallback and DEBUG mirror cache used the original pinned hashes;
no download checks, license/source requirements or test assertions were relaxed:

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

The temporary `.gitweave-playwright.config.js` imported the existing config and
changed only server commands/base URLs to free ports 18000/18001. Both projects,
all tests, workers, assertions and budgets were preserved. The root server command
was `python3 -m http.server 18000 --bind 127.0.0.1 --directory web`; the project
server command was `python3 target/smsw-serve-project.py`. That temporary copy of
`tests/browser/serve_project.py` used this worktree's `web/` path and port 18001,
retaining the temporary project symlink and root-asset rejection. The existing
port-8000 listener was left alone. The temporary config was removed after checks.

Local review checked the complete diff against Issue #39: CPU-local/reset status,
widths, atomicity, fault vector/frame/retry, mapping, VM isolation, source/license
provenance, identical native/Wasm image, CI and regression preservation. All 26
inventory categories retain their supplied statuses and full required scopes:
**partial** cpu-v30, cpu386, protected-mode, memory, pic, pit, dma, keyboard, mouse,
fdc, gdc, egc, pegc, text-cg, display-timing, opn, opna, bios, lio, integration;
**unimplemented** paging, fpu, rtc; **implemented, unverified** ide, grcg, pcm86.
Guest status writers, MOV control-register instructions, LMSW, CLTS, CR0/PE entry
and exit, descriptor lookup/caches/validation, CPL/privilege transitions, protected
gates, LDTR/TR, task-state behavior, protected execution, paging and x87 execution
remain outside this change. General exception/restart coverage remains partial.
ET readback and this bounded probe do not establish x87 support or commercial
compatibility. No PR, CI success or merge is claimed. No post-merge checks are
required by Issue #39.

## Issue #41: 8259A OCW2 priority (2026-10-23)

[Issue #41](https://github.com/takahirox/pc98-rust/issues/41) adds per-controller
programmable priority and rotation to the existing PIC request/acknowledgement
abstraction. Arbitration and nonspecific EOI now use the programmed order,
including after reordering active nested ISR bits. Ordinary EOIs preserve order;
rotating specific/nonspecific EOIs update the lowest line. ICW4 AEOI rotation
occurs only at a successful acknowledgement. ICW1, construction and Machine reset
restore IR0 highest and automatic rotation disabled. See the
[hardware contract and remaining gaps](hardware.md#8259a-ocw2-priority-subset).
The authoritative reference is [Intel 8259A, Figure 8 and initialization,
priority/EOI sections, printed pp. 9–16](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=9).

Before editing `src/devices.rs`, the Issue's unchanged minimal reproduction was
added as `tests/pic.rs::set_priority_minimal_reproduction` and executed:

```sh
cargo test --locked --test pic set_priority_minimal_reproduction
```

It failed (exit 101): actual `Some(8)`, expected `Some(9)`; one failed test.
`git diff 5a2dbf1 HEAD -- src/devices.rs` was empty before the implementation
commit, confirming the upstream PIC matched the Issue's evaluation baseline.
The identical test passes with the implementation. No regression assertion or
execution budget was weakened.

The 13 original tests in [tests/pic.rs](../tests/pic.rs) cover every programmed
lowest line and OCW2 family; wraparound and competing requests; masked retention;
nested ISR blocking even when an ISR line is masked; reordered nonspecific EOI;
specific/rotating EOI; no-op and empty ISR commands; ICW4 AEOI enable/disable,
passive status reads and failed acknowledgement; controller/Machine isolation;
ICW1 restoration and reset. Bus tests program master `00/02` and slave `08/0A`
on both profiles, independently change priority/rotation, check cascade ISR,
exercise both orders of separate EOIs, and require deferred delivery without
lost requests.

The original MIT [guest/pic.asm](../guest/pic.asm) boots directly through the
synthetic IPL, without DOS or third-party guest software. The offline
[preparation script](../tools/prepare_pic_fixture.py) assembles it with
`nasm -f bin guest/pic.asm -o pic.bin`, verifies the checked-in
[manifest](../fixtures/pic-manifest.json), and retains the source and MIT license
alongside the generated image outside `web/`. CI prepares this fixture before
native and browser tests. SHA-256 hashes:

- Source: `d11ac742d361a5ce6ef0c711590906f9291b71b5383efab0cd968b05ab768d91`.
- Boot sector: `43eb61fe159ffc87bf1f84a277cde693e95eb5823a6c3efc39a0fcbff534c4ae`.
- Image: `3c74e3c28cdb5d7ba9909f7e4d2ed5a1d84c57ba435f4a031b7946a716b41db8`.

The guest installs hardware timer/keyboard handlers, programs PIT channel 0 in
one-shot mode, sets IR0 lowest, and publishes phases 1, 2, 3 at `8001h` with IF
clear. The host uses only existing key input (Return down/up/down), never writes
guest memory, and steps the real CPU. Before each STI, the guest confirms both
IRR bits (`IRR & 3 == 3`) and records `03h` at `8010h..8012h`. The first keyboard
handler sends rotating nonspecific EOI, making IR1 lowest. The second timer
handler sends rotating specific EOI, making IR0 lowest. Other EOIs preserve order.
The exact hardware delivery trace at `8008h` is `01 00 00 01 01 00`, and ISR
snapshots at `8014h` are `02 01 01 02 02 01`. Keyboard bytes are `1C 9C 1C`.
The guest compares the full result/guard block before writing `PIC PASS` to text
VRAM; no software INT substitutes for hardware delivery.

Both native and production release-Wasm tests boot the identical disk on VM and
Ap before and after Machine reset; browser projects cover `/` and `/pc98-rust/`.
Every boot requires `PIC PASS`, no `FAIL` or host error, unchanged disk bytes,
all 32 result/guard bytes at `8000h`, and at most 2,000,000 scheduler slots:

```text
a50406a51c9c1ca5010000010100a5a5030303a5020101020201a5a5a5a5a5a5
```

Environment: Darwin 25.6.0 arm64, Homebrew Rust/Cargo 1.94.0, NASM 3.02,
Python 3.13.13, Node 24.12.0, npm 11.6.2, locked Playwright 1.56.1 Chromium;
production Wasm uses the repository's `build_web.py` rustup compiler selection.

| Exact command | Outcome |
| --- | --- |
| `python3 tools/prepare_pic_fixture.py --update-manifest` | Created original provenance/result manifest |
| `python3 tools/prepare_pic_fixture.py` | Passed; offline source/boot/image hashes and exact result contract reproduced |
| `python3 tools/prepare_dma_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_gdtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_idtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_smsw_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_freedos.py` | Passed; pinned binaries and complete matching sources retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed; existing manifests and licensed sources retained |
| `python3 tools/prepare_cpu_fixture.py` | Initial HTTP 503; passed after checksum-verified mirror cache preparation below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Initial HTTP 503; passed after checksum-verified mirror cache preparation below |
| `cargo test --locked --test pic` | Passed: 13 tests, zero skipped, including four native probe boots |
| `cargo test --locked` | Passed: all 153 native integration tests, zero skipped; unit/doc harnesses also passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `python3 tools/build_media.py --check` | Passed; firmware/media hashes unchanged |
| `python3 tools/build_font.py --check` | Passed; CG hash unchanged |
| `npm ci` | Passed; zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm run build` | Passed; release Wasm 448384 bytes and ten-asset site audit |
| `npx playwright test tests/browser/pic.spec.js` | Server startup failed: default port 8000 occupied |
| `npx playwright test --config=target/pic-validation/playwright.config.mjs tests/browser/pic.spec.js` | Passed: all four PIC browser cases (eight boots), zero skipped |
| `npm test -- --config=target/pic-validation/playwright.config.mjs` | Passed: build/media/font checks, release Wasm/site audit and all 58 browser tests in 27.5 seconds, zero skipped |
| `git diff --check` | Passed |

An initial full native run reached CPU386 with 49 passes and one missing-fixture
failure because CPULEVEL preparation had returned HTTP 503. Other unaffected
fixture preparation continued. Alternate ibiblio hostnames returned 503 or
connection refusal and the attempted GWDG paths returned 404; no unverified
archive was used. The previously documented Princeton mirror provided the exact
pinned archives; the original scripts then completed without changes:

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

The temporary alternate-port config imported `playwright.config.js`, retained
both complete projects and every assertion/budget, and changed only server URLs,
commands and working directories to this worktree and ports 18000/18001. Its
`testDir` resolved to the original `tests/browser/`. Root used
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`; Pages used
`python3 target/pic-validation/serve_project.py`, a temporary copy of the original
server with this worktree's `web/` path and port 18001, preserving its symlink and
root-asset rejection. The first temporary config attempt served from the wrong
working directory (404 during setup); it was stopped and corrected before tests
ran. Existing listeners were left untouched. Temporary files/logs are ignored
under `target/pic-validation/`; the normal checked-in config is unchanged.

Local diff review covers implementation, native and guest assertions, legal
provenance, identical native/Wasm image, reset, CI preparation and regression
preservation. Remote CI and independent PR review were not run in this node;
no push, PR, comment or merge was performed. No post-merge verification is
required by Issue #41.

All 26 inventory categories and their full required scopes retain their supplied
statuses: **partial** cpu-v30, cpu386, protected-mode, memory, pic, pit, dma,
keyboard, mouse, fdc, gdc, egc, pegc, text-cg, display-timing, opn, opna, bios, lio,
integration; **unimplemented** paging, fpu, rtc; **implemented, unverified** ide,
grcg, pcm86. PIC remains partial, including polling, special-mask mode, special
fully nested mode, general ICW sequencing/configuration, alternate cascade
wiring, edge/level electrical modeling and spurious-interrupt fidelity. Bounded
probes and game reports do not establish full hardware accuracy or commercial
compatibility.

## Issue #43: 8259A OCW3 special-mask mode (2026-10-23)

Implemented independent OCW3 ESMM/SMM state on both existing PIC instances.
ESMM qualifies mode changes independently of RR/RIS status selection. A shared
eligible-ISR calculation excludes masked service levels from special-mask
arbitration and nonspecific EOI, including rotating EOI. Specific EOI still
addresses masked levels. Empty eligible sets do not rotate. ICW1, construction
and Machine reset disable special-mask mode; mode changes preserve request,
service, mask, programmed priority and AEOI state. See the
[implemented subset and remaining gaps](hardware.md#8259a-ocw3-special-mask-subset)
and [Intel's OCW3/EOI and special-mask descriptions, printed pp. 13–16](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=13).

Before modifying `src/devices.rs`, the exact original Issue regression was added
to `tests/pic.rs` and executed against the unchanged upstream implementation:

```sh
cargo test --locked --test pic smm_minimal_reproduction -- --exact
```

It failed at the post-`68h` arbitration assertion (`left: None`,
`right: Some(1)`), exit 101: zero passed, one failed. After implementation, the
same command passed: one passed, zero failed. The selected checkpoint contains
the PIC implementation from evaluation baseline
`ab55c36ee99ec60a0ae71da466c30c2fc779a953`; intervening node commits carry Issue
metadata. The original 13 PIC tests, including the normal-mode masked-ISR case,
and all original OCW2 probe/source/manifest/browser assertions were verified
byte-for-byte unchanged.

Seven added native tests cover the minimal reproduction, all eight priority
origins with nested ISR and competing requests, mask and mode transitions,
ordinary/rotating/specific EOI, empty eligible sets, combined `6Ah/6Bh/4Ah/4Bh`,
ESMM=0, passive repeated reads, ICW1, construction, reset and instance isolation.
Assertions check exact IRR/ISR/IMR and observable delivery order. Empty-eligible
rotation checks additionally clear ISR with nonrotating specific EOIs and unmask
all requests, exposing even an erroneous rotation to a masked line. Both profiles
exercise master `00h/02h`, slave `08h/0Ah`, and slave special-mask delivery through
master IR7 with explicit master EOIs and independent controller modes.

The separate original MIT [guest/pic_smm.asm](../guest/pic_smm.asm) assembles
offline with `nasm -f bin guest/pic_smm.asm -o pic_smm.bin`. The
[preparation script](../tools/prepare_pic_smm_fixture.py) checks the
[manifest](../fixtures/pic-smm-manifest.json) and retains assembly, MIT license
and manifest alongside `fixtures/pic_smm/pic_smm.hdm`, outside `web/`. CI prepares
it before native/browser acceptance. It needs no DOS, proprietary ROM or guest
software. Source SHA-256:
`8a98e7cf9c6cee7996b5a203a7cdc8b696c7452d4e226c2bde4ad8aa6f64007c`;
image SHA-256:
`271e962902092a9ff6649892f7d96778a8975ebde2ff7f598f01dddc6ceb6a1c`.

The guest programs a one-shot PIT, sets IR0 lowest, and announces phase 1 at
`0x8001`. Hosts send one physical Return-down event through `bus.key` or browser
keyboard input. They never request IRQs or write guest memory/results. The guest
confirms IRR bits 0 and 1 together before STI. Its keyboard handler masks its own
ISR1, observes 256 iterations with IF set in normal mode, and records the timer
still pending and its handler count zero. It enables special-mask mode, admits
the timer as a nested hardware interrupt and records exactly one timer entry.
The nested handler sends nonspecific EOI, preserving masked ISR1; the outer
handler records ISR1 still set and finishes with specific EOI.

Required trace is `01 00 01` at `0x8008`; ISR snapshots are `02 03 02` at
`0x800c`; timer counts are `00 01` at `0x8006`. Guest and hosts compare all 32
result/guard bytes at `0x8000`:

```text
a50203011c030001010001a5020302a501fe0000a5a5a5a5a5a5a5a5a5a5a5a5
```

The identical checked image passed four native boots (VM and Ap, before/after
reset) and eight production release-Wasm browser boots (both profiles, both URL
roots, before/after reset). Every boot required `PIC SMM PASS`, no `FAIL` or
host/browser error, exact results and guards, unchanged disk bytes and completion
within 2,000,000 scheduler slots. The existing PIC probe and CPU interrupt-shadow,
IDTR, BIOS timer, DOS/application, VM and game assertions and budgets remain intact.

Environment: Darwin arm64, Homebrew Cargo 1.94.0, Python 3.13.13, NASM 3.02,
Node 24.12.0, locked Playwright 1.56.1 Chromium; production Wasm uses the existing
`build_web.py` compiler selection.

| Exact command | Actual outcome |
| --- | --- |
| `python3 tools/prepare_pic_smm_fixture.py --update-manifest` | Created original provenance/hash/result manifest |
| `python3 tools/prepare_pic_smm_fixture.py` | Passed; offline source, boot, image and exact results reproduced |
| `python3 tools/prepare_dma_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_gdtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_idtr_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_smsw_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_pic_fixture.py` | Passed; existing hash unchanged |
| `python3 tools/prepare_freedos.py` | Passed; checksum-pinned binaries and complete matching sources retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed; established manifests and provenance retained |
| `python3 tools/prepare_cpu_fixture.py` | Initial upstream HTTP 503; passed after checksum-verified mirror preparation below |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed using checksum-verified mirror cache below |
| `cargo test --locked --test pic` | Passed: 20 tests, zero skipped, including both original hardware probes |
| `cargo test --locked` | Passed: 160 native integration tests, zero skipped; unit/doc harnesses also passed |
| `cargo clippy --all-targets -- -D warnings` | Initial identity-operation warning in a new test fixed; final check passed |
| `cargo fmt --check` | Passed after `cargo fmt` |
| `python3 tools/build_media.py --check` | Passed in `npm test`; existing firmware/media hashes unchanged |
| `python3 tools/build_font.py --check` | Passed in `npm test`; CG hash unchanged |
| `npm ci` | Passed; zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Build/release Wasm/site audit passed; browser startup failed because default port 8000 was occupied |
| `npm test -- --config=target/pic-smm-validation/playwright.config.mjs` | Passed: release Wasm (448475 bytes), ten-asset audit, all 62 browser tests in 33.1 seconds, zero skipped |
| `git diff --check` | Passed |

The CPULEVEL upstream returned HTTP 503. The previously documented mirror
supplied exact pinned archives; the original preparation scripts completed
without changes after this checksum-verified cache preparation:

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
```

The temporary browser config imports the checked-in config, preserves both
complete projects and all tests/assertions/budgets, and changes only server
ports, URLs and worktree paths. Root uses
`python3 -m http.server 18000 --bind 127.0.0.1 --directory web`; Pages uses
`python3 target/pic-smm-validation/serve_project.py`, a copy of the existing
project server with this worktree's `web/` path and port 18001, preserving its
symlink and root-asset rejection. Existing listeners were left untouched.
Temporary configuration is ignored under `target/pic-smm-validation/`.
After final review strengthened the empty-eligible rotation assertions, focused
PIC tests, Clippy and formatting passed again.

Local review checked the complete implementation, native/browser/guest result
assertions, provenance, reset, CI fixture preparation and preservation of existing
regressions. No blocking findings remain. Remote CI and independent review were
not run in this node; no push, PR, comment or merge was performed. Issue #43 has
no required post-merge verification.

All 26 inventory categories retain their complete fixed scopes and supplied
statuses: **partial** cpu-v30, cpu386, protected-mode, memory, pic, pit, dma,
keyboard, mouse, fdc, gdc, egc, pegc, text-cg, display-timing, opn, opna, bios, lio,
integration; **unimplemented** paging, fpu, rtc; **implemented, unverified** ide,
grcg, pcm86. PIC remains partial: polling, special fully nested mode, general
ICW3 configuration, alternate cascade wiring, trigger/electrical behavior and
spurious interrupts remain missing or incomplete. Bounded probes and game reports
do not establish full hardware accuracy or commercial compatibility.


## Issue #43 review revision: automatic fixture mirrors (2026-10-23)

The blocking review finding identified acceptance CI stopping at CPULEVEL's
ibiblio HTTP 503 before any native/browser checks. DEBUG had the same failure.
Both preparation scripts now automatically try the previously validated FreeDOS
mirrors. CPULEVEL uses Princeton; DEBUG uses Princeton then GWDG. Each download
must match the original SHA-256 and 8 MiB size limit before it enters the cache.
A valid cache works offline; a corrupt cache still fails without replacement.
No source, license, canonical provenance URL, pinned hash or audited fixture
manifest was changed. CI also runs seven offline downloader regression tests
before fixture preparation, covering HTTP 503 recovery, checksum/size rejection,
exhausted mirrors, primary success and valid/corrupt caches.

A fresh worktree had neither downloaded archive cached. The ordinary preparation
commands below each received HTTP 503 from ibiblio, fetched the exact archive
from Princeton automatically and reproduced the existing image manifest. No
manual mirror fetch or cache preparation was used.

Environment: Darwin arm64, Python 3.13.13, Cargo 1.94.0, Node 24.12.0, NASM 3.02
and locked Playwright 1.56.1 Chromium. The complete CI preparation sequence ran
locally, followed by its acceptance checks:

| Exact command | Actual outcome |
| --- | --- |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py -v` | Passed: seven offline tests |
| `cargo test --locked` | Passed: all 160 native integration tests, zero skipped, including 20 PIC tests; unit/doc harnesses passed |
| `python3 tools/build_media.py --check` | Passed; existing media/firmware hashes reproduced |
| `python3 tools/prepare_dma_fixture.py`, `python3 tools/prepare_gdtr_fixture.py`, `python3 tools/prepare_idtr_fixture.py`, `python3 tools/prepare_smsw_fixture.py`, `python3 tools/prepare_pic_fixture.py`, `python3 tools/prepare_pic_smm_fixture.py` | All passed; existing source/image/result manifests reproduced |
| `python3 tools/prepare_freedos.py`, `python3 tools/prepare_font_fixture.py`, `python3 tools/prepare_dos_apps.py` | All passed with established pinned sources/licenses |
| `python3 tools/prepare_cpu_fixture.py` | Passed automatically after primary HTTP 503; image `b6d8381fa5fda6489d6ef53be81fb27b6b04cf64af24c2d24fd1cf94804a61a5` |
| `python3 tools/prepare_hdd_fixture.py`, `python3 tools/prepare_music_fixture.py` | Both passed |
| `python3 tools/prepare_device_fixture.py` | Passed automatically after primary HTTP 503; image `dff11e9e5acc3d8a98a26c0429f6aac69c7824441a4d055a8e1eb8dcfe90f51d` |
| `python3 tools/build_font.py --check` | Passed; existing CG hash reproduced |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `npm ci` | Passed; zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test -- --config=target/fixture-fallback-validation/playwright.config.mjs` | Passed: all 62 browser tests, zero skipped, in 36.4 seconds; release Wasm and ten-asset site audit passed |
| `git diff --check` | Passed |

Port 8000 was already occupied, so the temporary browser config imports the
checked-in config and changes only worktree paths/server working directories and
ports to 18000/18001, preserving both URL roots, all tests and budgets. Its first
attempt timed out because the temporary config resolved the server working
directory under `target/`; setting explicit worktree working directories fixed
that local harness error. The project-path server copies the existing symlink
server with only its web path and port changed. These temporary files are ignored
under `target/fixture-fallback-validation/`.

Remote GitHub Actions has not run against this revision: this node does not push
or merge. Publication and remote CI verification remain downstream work.

## Issue #45 8253 binary mode 2 (2026-10-23)

Implemented original binary mode-2 counting on the shared PIT counters, preserving
both PC-98 port banks and the existing 1,996,800/8,000,000 guest-clock conversion.
Mode encodings 2 and 6 set OUT high, await a complete count and consume a separate
initial load clock. Zero represents 65,536. Count 1 produces one low clock;
reload produces the rising edge. Completed rewrites update the reload register
without restarting the live period. Constant-time batch advancement finishes the
old period before skipping new periods and reports any rise, including a complete
low pulse between high endpoints. Bus programming detects a low-to-high OUT0
transition immediately; held-high output does not request another IRQ. Existing
PIC IRR latching coalesces elapsed periods without creating an interrupt backlog.
The [hardware contract](hardware.md#8253-binary-mode-2-subset) records the Intel
specifications and retained limits.

The 12 original tests in `tests/pit.rs` include independent count/OUT/edge traces
for divisors 2, 3 and 4, large-divisor boundary traces for 65,535 and encoded zero,
both mode encodings, every supported byte access form, a separate initial load,
completed/partial/multiple rewrites, every old-period boundary and an ordinary
latched read during counting. Single-clock advancement is compared with each
irregular batch's exact full device state and aggregate edge result, then a large
deferred-reload batch and `u32::MAX` scheduler-cycle advancement. Fractional and
zero advancement, both profiles and both port banks, channel isolation,
programming-induced edges, periodic timing, masks/IRR/ISR/EOI, IF/HLT/IRET and reset
of deferred reload/write/latch/fraction/request state are covered.

Negative controls temporarily restored only baseline
`77088025d060023d0bdcf1b4d5c4acc46db4e942`'s `src/devices.rs` and `src/bus.rs`.
`cargo test --locked --test issue45_negative -- --nocapture` ran two temporary
controls: the exact Issue reproduction failed (`left: 200`, `right: 91`), and
the new boot probe printed `PIT MODE2 FAIL`. The implementation was restored in a
`finally` block and the temporary test removed. All final focused tests passed.

The separate original MIT `guest/pit_mode2.asm` is assembled offline with
`nasm -f bin guest/pit_mode2.asm -o pit_mode2.bin`. Its
[checked manifest](../fixtures/pit-mode2-manifest.json) pins the source, 1024-byte
boot sector and complete disk hashes, license, command, result layout and budget.
Source SHA-256 is
`22723c6634cb3b0891da06612a24c07f97c50c8974e9289a208fd5100e58e03e`;
image SHA-256 is
`9ab69105bf22eef285787bf27e84df6ff562efe7d5fb1364e7c036049c03b8c2`.
The guest initializes every result/guard byte at `8000h..801Fh`, latches a running
60000-count period, rewrites 2000, and requires the new read to remain above 55000
and advance by less than 100. It reinitializes the PIC after programming to discard
setup transitions, then services exactly two periodic hardware IRQ0 entries using
EOI/IRET, checks ISR before/after EOI, and verifies the new period's count is at
most 2000. Guest and hosts require all 32 bytes:
`a501020101000001a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5`.
Hosts only boot, step and read; they inject neither IRQs nor results.

VM and Ap each booted this identical image natively before and after reset, and
in production release Wasm at `/` and `/pc98-rust/` before and after reset.
All twelve boots completed in **32,000 scheduler slots each**, within the declared
2,000,000-slot maximum. Each required `PIT MODE2 PASS`, no `FAIL`, host or browser
error, exact results/guards and unchanged disk bytes. The fixture preparation is
registered in existing CI; the browser suite discovers its four added cases.

Exact checks in this macOS arm64 worktree:

| Command | Outcome |
| --- | --- |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | Passed: 7 offline downloader tests |
| `python3 tools/prepare_dma_fixture.py` | Passed |
| `python3 tools/prepare_gdtr_fixture.py` | Passed |
| `python3 tools/prepare_idtr_fixture.py` | Passed |
| `python3 tools/prepare_smsw_fixture.py` | Passed |
| `python3 tools/prepare_pic_fixture.py` | Passed |
| `python3 tools/prepare_pic_smm_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode2_fixture.py --update-manifest` | Created the new checked manifest from original source |
| `python3 tools/prepare_pit_mode2_fixture.py` | Passed: reproduced and checked manifest, retained assembly/license beside test image |
| `python3 tools/prepare_freedos.py` | Passed: legal pinned guests and complete sources retained; existing image hashes unchanged |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed after primary HTTP 503 using existing automatic pinned mirror |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed after primary HTTP 503 using existing automatic pinned mirror |
| `python3 tools/build_media.py --check` | Passed: existing media/firmware hashes reproduced |
| `python3 tools/build_font.py --check` | Passed: existing CG hash reproduced |
| `cargo test --locked --test pit -- --nocapture` | Passed: initial 11 focused tests, four native probe boots at 32,000 slots each |
| `cargo test --locked --test pit` | Passed: final 12 focused tests |
| `cargo test --locked` | Passed: all 172 native integration tests; zero failed or ignored; unit/doc targets also passed |
| `cargo clippy --all-targets -- -D warnings` | Initial trace tuple type-complexity warning fixed with a local `Sample` alias; final run passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed: zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Release Wasm build, media/font reproducibility and ten-asset site audit passed; browser servers could not bind occupied port 8000 |
| `npm test -- --config=.issue45-playwright.config.js` | Passed: all 66 browser tests, zero skipped, in 32.9 seconds; production release Wasm build and site audit passed |
| `git diff --check` | Passed |

The temporary browser config imported the normal Playwright config and changed
only server ports/base URLs to 61080/61081. Its project server copied the existing
symlink server with only port 8001 changed to 61081, preserving root-asset
rejection. Both complete projects, assertions, guests and execution budgets were
preserved. Existing listeners were left alone. Both temporary files were removed
after the run. Existing PIT/beeper, BIOS interval, PIC/SMM, interrupt-shadow,
DOS/application, VM and game assertions and budgets were not relaxed.

All 26 inventory categories retain their complete fixed scopes and supplied
statuses: **partial** cpu-v30, cpu386, protected-mode, memory, pic, pit, dma,
keyboard, mouse, fdc, gdc, egc, pegc, text-cg, display-timing, opn, opna, bios, lio,
integration; **unimplemented** paging, fpu, rtc; **implemented, unverified** ide,
grcg, pcm86. PIT remains partial: other mode-specific behavior, external gates,
BCD, divisor-1 accuracy, comprehensive latch/sequencing and mode-accurate beeper
mixing remain outstanding. These bounded probes and the existing six-game
reports do not establish full accuracy or commercial compatibility. No
proprietary ROM was acquired. Remote CI and independent review were not run;
this node does not push, open PRs, comment or merge. No post-merge verification
is required by Issue #45.

## Issue #47 8253 binary mode 3 (2026-10-23)

Implemented binary mode-3 initial loading and steady counting on all three
counters, both timer port banks and both profiles. Encodings 3/7 select the same
mode, zero encodes 65,536, and a complete initial count loads on a separate clock.
Even divisors subtract two; odd divisors first subtract one in the high half and
three in the low half. Full-divisor reloads toggle OUT. Bounded phase arithmetic
retains any rising edge across large batches, including hidden transitions with
identical endpoint levels, and preserves the scheduler conversion and fractional
remainder. The existing bus programming-edge detection and PIC latch now receive
mode-3 rises; falling edges and held-high output do not request IRQ0. Only the
mode-3 beeper polarity consumer changes to OUT; enable, amplitude, sample rate,
queue bound and other modes' mixer behavior remain unchanged. See the
[hardware contract and Intel sources](hardware.md#8253-binary-mode-3-subset).

`tests/pit.rs` retains all 12 mode-2 tests and adds 10 mode-3 tests. Independent
explicit traces cover divisors 2/3/4/5, both encodings, all channels, ordinary
reads and a separate load. Boundary/access checks include LSB-only, MSB-only,
paired, incomplete initial writes, 65,535 and every zero encoding. Latches remain
frozen across continued counting into the low half. Single clocks and irregular
batches compare entire device state and aggregate edges, including many periods,
hidden transitions, fractional/zero advancement and `u32::MAX` scheduler cycles.
Both profiles/banks exercise all channels, programming rises, falling-edge
silence, masks/IRR/ISR/EOI, request coalescing and mode-2/3 reprogramming. CPU tests
cover IF-clear/HLT delivery followed by EOI/IRET; reset checks active and partial
programming, latches, fraction and pending IRQ state. A mixer regression checks
both even and odd low-half reloads where a raw-count threshold gives the wrong
polarity. The existing square-wave frequency, silence, enable and queue-bound
regressions pass. `tests/core.rs::pit_keyboard_and_beeper` acknowledges the setup
IRQ separately and now expects the documented mode-3 count 6 after 998 PIT
clocks (load, 500 high clocks, 497 low clocks), retaining keyboard/beeper checks.

Negative controls used exactly baseline
`e382535d3d11d2e137d9e0feeda226b66f9ef468`'s `src/devices.rs` and `src/bus.rs`,
restored afterward in a Python `finally` block. Actual commands/results:

| Command on baseline sources | Result |
| --- | --- |
| `cargo test --locked --test pit issue47_initial_load_reproduction -- --nocapture` | Expected failure, exit 101: actual `(3, false)`, required `(4, true)` |
| `cargo test --locked --test pit original_mode3_boot_probe_before_and_after_reset_in_both_profiles -- --nocapture` | Expected failure, exit 101: guest printed `PIT MODE3 FAIL`, subtract-two proof unset |

Both pass with the implementation restored. Initial new test setup failures
(latch sampling too close to the low-half reload, accumulated fractional clocks
in bus timing, and a masked setup IRQ in the core regression) were corrected
without weakening the hardware expectations or existing assertions.

The original MIT [probe](../guest/pit_mode3.asm) builds offline with
`nasm -f bin guest/pit_mode3.asm -o pit_mode3.bin`. The checked
[manifest](../fixtures/pit-mode3-manifest.json) pins provenance, source/boot/image
hashes, assembly command, exact result/guard layout and the 2,000,000-slot budget.
Source SHA-256 is
`2db96377924049700e7c1aac368ce5df8554d642a71a3bbbc82860eb65f3e9fa`;
image SHA-256 is
`7c373364f171b90e86123777f6d99b8c50bc65adab4f0f634d323603df300a44`.
The guest initializes all 32 bytes at `8000h..801Fh`, latches two running even
counts from divisor 60000 separated by 500 LOOP instructions, and requires a
1900..2200 decrement, distinguishing subtract-two from the baseline countdown.
A new alias-7 mode word programs divisor 2000, followed by PIC initialization to
discard setup edges. Two periodic hardware IRQ0 entries check ISR=1, a nonzero
even count <=2000, EOI, ISR=0 and IRET. Guest and hosts compare all result/guard
bytes:
`a501020101000001a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5`.
Hosts only boot, step and inspect; they inject no IRQs or results.

The identical image passed on VM and Ap before/after reset, natively and in
production-release Wasm at `/` and `/pc98-rust/`. All **12 boots completed in
4,000 scheduler slots each**, below the fixed 2,000,000-slot budget. Every run
required exact results/guards, `PIT MODE3 PASS` without `FAIL`, unchanged disk
bytes and no core/host/browser errors. Existing CI now prepares this fixture;
the browser suite discovers four new cases, each including reset.

Exact checks in this macOS arm64 worktree:

| Command | Outcome |
| --- | --- |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | Passed: 7 offline downloader tests |
| `python3 tools/prepare_dma_fixture.py` | Passed |
| `python3 tools/prepare_gdtr_fixture.py` | Passed |
| `python3 tools/prepare_idtr_fixture.py` | Passed |
| `python3 tools/prepare_smsw_fixture.py` | Passed |
| `python3 tools/prepare_pic_fixture.py` | Passed |
| `python3 tools/prepare_pic_smm_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode2_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode3_fixture.py --update-manifest` | Created checked manifest from original source |
| `python3 tools/prepare_pit_mode3_fixture.py` | Passed: reproduced hashes and retained assembly/license/manifest beside image |
| `python3 tools/prepare_freedos.py` | Passed: pinned legal binaries and complete sources retained; existing image hashes unchanged |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed via existing checksum-pinned mirror after primary HTTP 503 |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed via existing checksum-pinned mirror after primary HTTP 503 |
| `python3 tools/build_media.py --check` | Passed: existing firmware/media hashes reproduced |
| `python3 tools/build_font.py --check` | Passed: existing CG hash reproduced |
| `cargo test --locked --test pit -- --nocapture` | Passed: 22 tests, including four mode-3 native boots at 4,000 slots each |
| `cargo test --locked --test pit` | Passed: final all-channel port read assertions, 22 tests |
| `cargo test --locked --test core pit_keyboard_and_beeper` | Passed with corrected mode-3 trace and separate setup IRQ accounting |
| `cargo test --locked` | Passed: 182 integration tests across 17 targets, zero failed or ignored; unit/doc targets also passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed: zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Release Wasm build and ten-asset site audit passed; browser server could not bind occupied port 8000 |
| `npm test -- --config=.issue47-playwright.config.js` | Passed: release Wasm build/site audit and all 70 browser tests, zero skipped, 42.0 seconds |
| `git diff --check` | Passed |

The temporary Playwright config imported the normal config and changed only
ports/base URLs to 62080/62081. Its project server copied the existing symlink
server with only port 8001 replaced by 62081, preserving rejection of root asset
URLs. Both complete projects, assertions and budgets were preserved; existing
listeners were left alone. Both temporary files were removed after the run.
Existing mode-2, PIC/SMM, BIOS interval, CPU interrupt-shadow, DOS/application,
VM and game assertions and budgets pass without relaxation. No checks were
skipped. Remote CI and independent review are unestablished.

All 26 inventory categories retain their complete fixed scopes and statuses:
**partial** cpu-v30, cpu386, protected-mode, memory, pic, pit, dma, keyboard, mouse,
fdc, gdc, egc, pegc, text-cg, display-timing, opn, opna, bios, lio, integration;
**unimplemented** paging, fpu, rtc; **implemented, unverified** ide, grcg, pcm86.
PIT remains partial: active mode-3 rewrites without a new mode word, mode-0
continuation after terminal count, mode-specific behavior for 1/4/5,
GATE/retriggering, BCD, divisor 1, comprehensive latch/access sequencing and fuller
beeper timing remain outstanding. Protected execution, paging and basic FPU
remain required goals. Limited probes and six-game reports do not establish full
accuracy or commercial compatibility. No commercial ROM or guest was acquired.
No push, PR, comment or merge was performed. Issue #47 requires no post-merge
verification.

## Issue #49: 8253 binary mode 0 (2026-10-23)

Implemented the bounded binary mode-0 lifecycle in original Rust for all three
counters on VM/Ap, with GATE held high. A fresh mode word lowers OUT and clears
active/partial/latch state. Complete initial programming schedules a separate
load clock. Counting decrements to zero, raises OUT once, then continues modulo
65,536 with OUT high. Constant-time batch arithmetic preserves the existing
clock conversion and fractional remainder. The existing bus/PIC rise latch is
unchanged; no beeper mixer or resampler code changed.

The eight added [PIT tests](../tests/pit.rs) cover the reported `36h,30h,04h,00h`
reproduction; explicit count/OUT/edge traces for 1/2/4; 65,535 and encoded-zero
boundaries; all channels and supported LSB/MSB/pair forms; incomplete pairs and
load clocks; ordinary snapshot reads across terminal count and wraps; fresh
mode-word transitions; channel isolation; reset during partial, pending-load and
post-terminal states; single-clock versus irregular batches, zero advancement,
fractional cycles and split/unsplit `u32::MAX` advances. Both port banks on both
profiles check masking, retained setup and terminal IRR across falling OUT,
acknowledgement, ISR/EOI, rearming, no repeated requests across wraps, and CPU
IF-clear/HLT followed by delivery, EOI and IRET. Existing mode-2/3 programming
rises are accounted for separately. Undefined pre-load counts are not asserted.

Negative controls temporarily used exactly
`69e60a951c1a659ab67bf535a8e4bb5b97c43383:src/devices.rs`, restored in a Python
`finally` block before building the fixed native/Wasm core:

| Baseline command | Actual result |
| --- | --- |
| `cargo test --locked --test pit issue49_mode0_initial_load_reproduction -- --nocapture` | Expected failure, exit 101: actual `(3, true)`, required `(4, false)` |
| `cargo test --locked --test pit original_mode0_boot_probe_before_and_after_reset_in_both_profiles -- --nocapture` | Expected failure, exit 101: `PIT MODE0 FAIL`, one entry handled but post-terminal advancing-count proof unset |

Both pass with the fixed implementation. No other production source needed
replacement: the existing bus/PIC code is unchanged from that baseline.

The original MIT [guest probe](../guest/pit_mode0.asm) assembles offline with
`nasm -f bin guest/pit_mode0.asm -o pit_mode0.bin`. Its checked
[manifest](../fixtures/pit-mode0-manifest.json) pins the source, boot/image hashes,
license, exact results/guards and fixed 2,000,000-slot maximum per boot. Source
SHA-256 is `53d34a44601716fa9f0ff335863e310e1c7190a1b64fe88213a7b2a4f7d8b43e`;
image SHA-256 is `c7d4ca54421543ee77ab580310717201e865a95d9c88273f332a90a15136fddc`.
The preparation script retains assembly, license and manifest beside the image
outside `web/`, and existing CI prepares it before testing.

The guest lowers OUT and initializes the PIC to discard firmware/setup requests,
then arms divisor 2000 twice using fresh mode words. Each arm has a bounded
6000-iteration wait and exactly one hardware IRQ0 with ISR=1, EOI, ISR=0 and
IRET. Two ordinary latched post-terminal counts must be at least 60000, with the
first below 65535; 500 LOOP instructions must decrease the count by 900..1100.
It then remains unmasked for 40000 LOOP instructions (over 65536 PIT clocks),
rejecting additional IRQ entries and checking IRR0=ISR0=0 before rearming. Both
the guest and hosts require all 32 result/guard bytes at `8000h..801Fh`:
`a5020101000001010101a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5`.
Hosts only boot, step and inspect; no IRQs or results are injected.

The identical source-built image passed on VM/Ap before and after reset,
natively and through production-release Wasm at `/` and `/pc98-rust/`. All
**12 boots completed in 84,000 scheduler slots each**. Each required exact
results/guards, `PIT MODE0 PASS` without `FAIL`, unchanged disk bytes and no
core/browser errors, within the original two-million-slot budget.

Actual checks in this macOS arm64 worktree:

| Command | Outcome |
| --- | --- |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | Passed: 7 tests |
| `python3 tools/prepare_dma_fixture.py` | Passed |
| `python3 tools/prepare_gdtr_fixture.py` | Passed |
| `python3 tools/prepare_idtr_fixture.py` | Passed |
| `python3 tools/prepare_smsw_fixture.py` | Passed |
| `python3 tools/prepare_pic_fixture.py` | Passed |
| `python3 tools/prepare_pic_smm_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode2_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode3_fixture.py` | Passed |
| `python3 tools/prepare_pit_mode0_fixture.py --update-manifest` | Created checked manifest from original source |
| `python3 tools/prepare_pit_mode0_fixture.py` | Passed: reproduced pinned image/source hashes |
| `python3 tools/prepare_freedos.py` | Passed: pinned legal binaries and matching complete sources retained |
| `python3 tools/prepare_font_fixture.py` | Passed |
| `python3 tools/prepare_dos_apps.py` | Passed |
| `python3 tools/prepare_cpu_fixture.py` | Passed via existing checksum-pinned mirror after primary HTTP 503 |
| `python3 tools/prepare_hdd_fixture.py` | Passed |
| `python3 tools/prepare_music_fixture.py` | Passed |
| `python3 tools/prepare_device_fixture.py` | Passed via existing checksum-pinned mirror after primary HTTP 503 |
| `python3 tools/build_media.py --check` | Passed: existing media/firmware hashes unchanged |
| `python3 tools/build_font.py --check` | Passed: existing CG hash unchanged |
| `cargo test --locked --test pit -- --nocapture` | Passed: 30 tests; mode-0 native boots each 84,000 slots |
| `cargo test --locked --test pit` | Passed: final retained-terminal-IRR and partial-transition/latch assertions, 30 tests |
| `cargo test --locked` | Passed: 190 integration tests across 17 targets, zero failed/ignored; unit/doc targets also passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `npm ci` | Passed: zero vulnerabilities |
| `npx playwright install chromium` | Passed |
| `npm test` | Release-Wasm build and ten-asset publication audit passed; browser server could not bind occupied port 8000 |
| `npm test -- --config=.issue49-playwright.config.js` (first run) | 73 passed, 1 failed: existing local VM keyboard test failed at `clock.pauseAt` with “Cannot fast-forward to the past”, before loading the emulator; all mode-0 cases passed |
| `npm test -- --config=.issue49-playwright.config.js` (complete rerun) | 73 passed, 1 failed: existing Pages VM mode-3 test hit the same `clock.pauseAt` error before loading the emulator; all mode-0 cases passed |
| `npm test -- --config=.issue49-playwright.config.js --workers=2` | Passed: production-release Wasm build, ten-asset audit and all 74 browser tests, zero skipped, 1.9 minutes |
| `git diff --check` | Passed |

The temporary config imports the normal Playwright config and changes only
ports/base URLs to 62082/62083. Its project server is the existing symlink server
with port 8001 replaced by 62083, preserving rejection of root asset URLs. All
projects, tests, assertions and execution budgets are retained. The final run
limits browser concurrency to two workers after two clock setup failures in
different existing tests with the default eight workers. Existing listeners
were left alone; both temporary files were removed after testing. No checks were
skipped. No emulator/test changes were made for the clock setup failures.
Existing PIT, PIC/SMM, BIOS interval, CPU-shadow, DOS/application, VM and game
assertions and budgets remain intact. Remote CI and independent review are
unestablished.

All 26 fixed inventory scopes and statuses remain unchanged: **partial** cpu-v30,
cpu386, protected-mode, memory, pic, pit, dma, keyboard, mouse, fdc, gdc, egc, pegc,
text-cg, display-timing, opn, opna, bios, lio, integration; **unimplemented** paging,
fpu, rtc; **implemented, unverified** ide, grcg, pcm86. PIT remains partial.
Count-only mode-0 rewrites, external GATE, modes 1/4/5, active mode-3 rewrites,
BCD, comprehensive latch/access corner cases, divisor-1 mode-2/3 behavior and
fuller audio remain outstanding; mode-0 audio is unverified. Protected execution,
paging and basic FPU remain required goals. Limited probes and six-game reports
do not establish full hardware accuracy or commercial compatibility. No
commercial ROM or guest was acquired. No push, PR, comment or merge was performed.
Issue #49 requires no post-merge verification.
