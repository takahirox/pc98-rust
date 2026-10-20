# Issue #3 pre-merge validation

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
