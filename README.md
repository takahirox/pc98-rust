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
Protected mode, DPMI, FPU and full 486 fidelity are not implemented.
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
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Build/check the publication tree and run Chromium at / and /pc98-rust/.
npm ci
npx playwright install chromium
npm test
```

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

`prepare_freedos.py` also builds `fixtures/freedos/hblank.hdm` from the original
MIT [horizontal-blank probe](guest/hblank.asm). `cargo test --test gdc` requires
this disk and verifies `HBLANK TRANSITION PASS` through guest DOS on both profiles,
within 16 million scheduler slots. Actual-browser tests run the same disk and
budget on real Wasm at both serving paths. The probe initializes video and polls
both `0060h` and `00A0h`; no game or ROM download is needed. See the
[fixed display timing contract](docs/hardware.md#device-ports-and-interrupts).

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
