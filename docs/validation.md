# Validation evidence

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
