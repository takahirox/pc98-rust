# Validation evidence

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
