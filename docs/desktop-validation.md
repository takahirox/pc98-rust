# Installed browser desktop (Issue #130)

A fresh browser visit selects Ap and starts the real FreeDOS(98) kernel and
FreeCOM from a writable 128 MiB IDE HDD. FDD1/FDD2 are empty. The library and
headless profile default remains VM; headless still selects the supplied image's
boot device. Native callers now use `Machine::set_boot_hdd(bool)` explicitly;
Wasm exposes `set_boot_source(0 /* floppy */ | 1 /* HDD */)`. Insert/eject never
changes that choice. Firmware tries only that device on reset; VM always uses
floppy while retaining the Ap preference and IDE bytes.

## Disk and delivery

`python3 tools/prepare_desktop.py` checks the existing pinned upstream binary and
complete source archives, assembles the unchanged GPL HDD boot code and original
MIT partition IPL, and builds a cleared FAT16 layout. No raw large intermediate
is allocated or published. The [manifest](../fixtures/desktop-manifest.json)
pins all inputs, guest files, geometry, template and logical image hashes.

| Field | Value |
| --- | --- |
| Logical HDI extent | 134,221,824 bytes (128 MiB payload + 4,096-byte header) |
| CHS / sector | 512 cylinders × 16 heads × 32 sectors × 512 bytes |
| Partition | LBA 512; 261,632 sectors; active PC-98 FAT16 |
| FAT | Two 128-sector copies; 4 KiB clusters; 512 root entries |
| Free space | 32,622 clusters = 133,619,712 bytes |
| Template transfer | 655,360 bytes = 0.4883% of full HDI transfer |
| Logical image SHA-256 | `3aac6b03526b7957e3764c6439321e5a742dcf22894ab07e6ac83df86c8726cb` |

All nonzero disk bytes are in a 64 KiB-aligned prefix; the remainder is defined
as zero. This deliberately simple sparse format needs neither a new archive
parser nor a decompressor. Browser startup verifies the prefix size/hash, feeds
it through the existing transactional HDI importer and supplies zero chunks for
the remaining extent. Rust stores only nonzero pages. JavaScript holds a bounded
prefix and 1 MiB zero/transfer buffers, never a full 128 MiB disk copy. The mounted
HDI uses the same read/write/export representation as local large HDIs.

Controls lock throughout startup/import/export, and dropped files cannot race an
in-progress operation. Failed geometry, checksum, HTTP/read or allocation leaves
previous media intact and cancels staging. Restore FreeDOS HDD retries startup or
replaces a custom disk with a clean desktop. Dirty HDD eject/replacement requires
confirmation; cancel preserves it and allows export. There is no browser storage
or upload. Reset preserves writes; reload starts clean; export/reimport saves them.

## Actual guest and browser evidence

`tests/browser/desktop.spec.js` uses production release Wasm, at `/` and
`/pc98-rust/`. It checks the exact kernel bytes at the genuine loader's
`0060:0000` handoff, then executes PROBE and ECHO via the real DOS shell, cold and
after reset. An unmocked animation test boots without any click or manual core
stepping and sends physical keyboard input to ECHO. The HDD-booted guest enumerates
`A: SASI1`, `B: FD0`, `C: FD1`; the BIOS drive table is `[80h,90h,91h]`.

The same running shell reads independently generated original FAT data through
actual FDC I/O in both floppies, swaps D88/raw media, verifies complete unchanged
exports (including a protected D88), ejects, observes the real DOS no-media
critical error, reinserts and retries. No insertion changes execution/text or
HDD preference. Explicit floppy demo boot and return to HDD exercise both IPLs.

Other cases cancel all three destructive HDD actions, create a file via guest
DOS, download the complete modified 128 MiB disk, reimport it, TYPE the file,
retain it across VM/Ap switches, restore a clean desktop, recover startup failures
and reject a competing drop while controls are locked. Existing media routing,
floppy-booted DOS FDD2, independent demos, CPU/device, 512 MiB/1 GiB and occupied
512 MiB round-trip regressions remain enabled.

## Measurements

Measured locally on macOS arm64 with the installed Chromium/Playwright 1.56.1,
release Wasm and the Python static servers. These are observations, not timing
gates or claims about deployed Pages performance. A raw 128 MiB transfer would
be 134,221,824 bytes; the desktop template transfers 655,360 bytes. Source
archives are linked downloads and are not fetched at startup.

Final navigation-through-mount observations: 83 ms at `/`, 87 ms at
`/pc98-rust/`; natural animation boot to prompt 4,953 ms and 4,916 ms. Wasm high-water memory after two DOS boots
and commands: 14,090,240 bytes; resident HDD pages: 458,752 bytes. Sampled renderer
peak RSS about 180 MiB and total browser RSS about 396 MiB. Process RSS includes
Chromium/framebuffer/runtime allocations and can count shared pages more than
once; 100 ms sampling is not an exact OS high-water measurement. Final steady and sampled peak renderer RSS were both 184,368 KiB; total
browser RSS was 405,104 KiB at `/` and 404,368 KiB at `/pc98-rust/`. Per-asset
Resource Timing is emitted by the tests for each serving root.

The measured startup resource bodies totalled **1,240,132 bytes** at either root:
HTML 4,250 bytes, CSS 3,356 bytes, JS 22,113 bytes, Wasm 552,196 bytes, manifest
2,857 bytes and template 655,360 bytes. This is local uncompressed HTTP body size,
excluding protocol overhead; deployed-site compression and latency are unmeasured.
The template saves more than 99.5% of the HDD network transfer. Existing local
large-disk tests continue to record import/export memory and timings separately.

## Sources and packaging

The [license audit](licenses.md#installed-browser-desktop-issue-130) describes the
GPLv2 section 3(a) accompanying source publication. `web/licenses.html` provides
same-site links to the complete matching kernel/boot and FreeCOM/SUPPL sources,
MIT helpers/configuration and all notices. The browser checks fetch those links
from both serving roots and verify their hashes.

`npm run check:site` permits exactly nineteen regular files: the previous ten
assets and the nine entries pinned in
[desktop-publication.json](../fixtures/desktop-publication.json). A corrupted
published source archive and an extra unapproved disk were each deliberately
introduced locally: both were rejected, then restored. The published helper
source archive was also unpacked in a clean temporary directory, supplied with
only the four checksum-pinned input archives and rebuilt: all nine publication
artifacts matched byte-for-byte. No test-only DOS disks or raw 128 MiB media ship.

## Checks

Validation commands: all fixture preparations listed in the acceptance workflow;
Python fixture-fetch tests; `cargo test --locked`; focused HDD regression after
the new boot-policy test; `cargo clippy --all-targets -- -D warnings`;
`cargo fmt --check`; release Wasm build and exact site audit through `npm test`;
complete Chromium suite with two workers at both roots; `git diff --check`.
Results on 2026-11-05:

- All original/GPL fixture preparations and seven Python fetch tests passed.
- Full native suite: 554 passed. The added boot-policy regression subsequently
  passed with the complete 17-test HDD target; 555 unique native cases covered.
- Clippy with warnings denied, formatting, final release Wasm build, nineteen-asset
  source/license audit and whitespace checks passed.
- Complete Chromium run: 224 cases at both roots, initially 217 passed, 3 intentional
  existing path-sampling skips and 4 setup failures. The large-HDI test had duplicate
  confirmation handlers; the VM86 firmware probe deliberately requires no HDD for
  its invalid-device assertion. Both setups were corrected without weakening guest
  assertions.
- Final rerun of desktop, large-HDI and VM86 firmware files at both roots:
  **27 passed, 3 intentional skips in 43.4 s**. This covers all four failures and the
  final startup/memory/source-link tests. Together the full run and corrected rerun
  cover all **221 active Chromium scenarios**. The initial full run took 7.6 minutes;
  focused reruns avoid repeating unrelated guest CPU/device suites.

No live deployment, private media, manual playtest or post-merge verification is
required for this Issue.
