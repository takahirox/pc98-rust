# pc98-rust

An original Rust real-mode PC-98 emulator, with a local browser demo. **Star
Trail** is a bootable guest game: its machine code draws text and bitmap VRAM,
reads PC-98 keyboard input, beeps, and saves through the emulated floppy controller.
The browser displays the Rust framebuffer and plays the core's clocked PCM.
There are no emulation dependencies, proprietary ROMs, font dumps, or server-side
execution.

The experimental profile targets a **PC-9801VM with 640 KiB RAM, V30 real mode,
640×400 display and three graphics planes**. Compatibility is the subset exercised
by the included guests and the pinned FreeDOS(98) smoke test. This is not a general
PC-98 compatibility claim. See [hardware, ports, firmware and limits](docs/hardware.md).

## Build and play

Install Rust through rustup (including the Wasm target), Python 3.10+, NASM 2.16+,
and Node.js 22+ for browser testing. The application itself has no npm runtime
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
floppy** or drop an image on the screen. Inserting drive 1 boots it; inserting drive
2 changes media without rebooting. **Eject** removes the selected disk. **Export
disk** downloads that drive's current bytes, preserving D88 headers and write
protection. Reinsert the export to restore saved disk data. Star Trail records the
last run's moves and score in sector 8; a new game starts at zero. There is no
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
hiding the tab pauses execution. Kanji ROM/FEP input is not implemented.

## Reproduce the checks

```sh
# Fetch checksum-pinned guest binaries AND matching complete source archives.
# This makes a minimal GPL guest disk; it downloads no emulator code.
python3 tools/prepare_freedos.py
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Build real Wasm, serve the shipped app, and run Chromium acceptance tests.
npm ci
npx playwright install chromium
npm test
```

`cargo test --test dos` deliberately fails if the legal guest fixture is missing;
it is never silently skipped. The fixture downloads require network access once.
Their checksums and sources are pinned in the preparation script. **PROBE.COM**
runs via the guest FreeDOS kernel and shell, including real INT 21h handling inside
DOS. The emulator has no host DOS implementation. The minimal fixture excludes
XMS drivers because this profile is real-mode V30, not a 286 machine. To try it in
the browser, insert `fixtures/freedos/minimal.hdm`, wait for `A:\>`, and type `probe`.
See [licensing and distribution](docs/licenses.md) before redistributing this disk.

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

[CI](.github/workflows/emulator.yml) performs the fixture preparation, native
checks, Wasm build and browser tests on pull requests before merge. Local test
coverage and execution evidence are in [validation](docs/validation.md). No
published site, post-merge deployment, human playtest or original hardware is
required. No FM/OPNA, EGC, protected mode, HDD/SCSI, NEC BASIC or Kanji font is
claimed. Unsupported CPU/BIOS operations report errors; unmapped ports return an
open bus and are recorded for headless diagnostics.

## Attribution and contributing

Original emulator, firmware, glyphs and Star Trail / PROBE sources are MIT licensed.
The hello-world boot sector and its source retain their separate MIT notice.
The downloaded FreeDOS/FreeCOM guests are GPLv2 or later and keep corresponding
source archives beside the generated image. See [file-level audit](docs/licenses.md).

- [Development flow and English collaboration convention](docs/development-flow.md)
- [Review and merge guidelines](docs/review-guidelines.md)
- [Issue template](.github/ISSUE_TEMPLATE/issue.md) and [PR template](.github/pull_request_template.md)
- [AI agent instructions](AGENTS.md)
