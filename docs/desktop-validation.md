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
| Free space | 32,611 clusters = 133,574,656 bytes |
| Template transfer | 655,360 bytes = 0.4883% of full HDI transfer |
| Logical image SHA-256 | `a994056731d0471cefac1c74da12d5f4ba372e41590e6f3e6aeb8e7f8de355b5` |

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

## Historical Issue #130 measurements

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

`npm run check:site` permits exactly twenty-four regular files: the original ten
assets and the fourteen entries pinned in
[desktop-publication.json](../fixtures/desktop-publication.json). A corrupted
published source archive and an extra unapproved disk were each deliberately
introduced locally: both were rejected, then restored. The published helper
source archive was also unpacked in a clean temporary directory, supplied with
only the four checksum-pinned input archives and rebuilt: all nine publication
artifacts matched byte-for-byte for Issue #130. The Issue #132 rebuild of all
fourteen artifacts is recorded below. No test-only DOS disks or raw 128 MiB media ship.

## Historical Issue #130 checks

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


## High-memory desktop (Issue #132)

The default Ap disk now loads the unmodified pinned PC-98 HimemX 3.34 before
`DOS=HIGH`, followed by the same FreeCOM command line. Restore FreeDOS HDD uses
this identical audited template. No CPU/BIOS/bus corrections or EMM386/UMB/EMS
assumptions were necessary. VM's built-in demos and every historical low-DOS
control retain their original bytes/configurations. An uploaded/exported HDD
keeps its own configuration; restoring the default explicitly replaces it.

### Independent conventional allocation

At the real FreeCOM prompt, run `MEMSIZE`. This original MIT program in
[guest/memsize.asm](../guest/memsize.asm) releases its COM tail with INT 21h/AH=4Ah,
retaining 256 paragraphs including its PSP, code and stack. AH=48h/BX=FFFFh must
fail with AX=8; the returned BX is then **successfully allocated** using AH=48h.
The program checks the block ends below A0000h, writes/reads both endpoint guards,
frees it with AH=49h, and requires the next largest-block query to match exactly.
It reports paragraphs and bytes and exits through DOS. The measurement excludes
its own 4 KiB allocation and environment, and includes FreeCOM's normal EXEC/XMS
swap behavior. It is a demonstrated contiguous allocation available to a DOS
program launched by the resident shell, not total free memory, XMS capacity or a
host allocator count. It does not claim the diagnostic's own memory is free.

| Same Ap profile (640 KiB conventional + 3 MiB extended) | Largest allocated paragraphs | Bytes |
| --- | ---: | ---: |
| Previous `DOS=LOW`, no manager | 31,116 | 497,856 |
| HimemX(98) + `DOS=HIGH` | 38,729 | 619,664 |
| Demonstrated improvement | 7,613 | 121,808 |
| Compatibility target | 34,560 | 552,960 |

Both native and production release-Wasm Chromium tests reproduce these exact
results on cold boot and reset, including two invocations at each prompt. The
baseline uses the same FAT16 disk, shell and diagnostic, changing only FDCONFIG
back to the original `DOS=LOW`/SHELL text with no DEVICE line; no historical
fixture is rewritten. Native observation records actual real-mode instructions
executed above 1 MiB with A20 enabled only in the high configuration. The driver
initializes through guest instructions and DOS reports 19 buffers / 19,836 bytes
in HMA. The functional diagnostic also checks DOS's INT 21h/3306h HMA flag and
that HMA is already owned, independently of the boot message.

`XMSCHECK`, from [guest/xmscheck.asm](../guest/xmscheck.asm), discovers the real
INT 2Fh XMS entry, queries A20, balances local enable/disable (the existing DOS
HMA lock keeps it enabled), allocates 16 KiB of XMS, copies an original pattern to
extended memory and back, and verifies its bytes. Out-of-block and invalid-handle
moves must be rejected without changing the destination; a subsequent valid
transfer must recover. It locks/unlocks the block, checks its extended physical
address, frees it and verifies XMS free capacity is restored. The pinned driver's
range rejection is error A7h (offset plus length exceeds block), invalid source
handle A3h, and a remaining A20 lock reports 94h. Native tests also require zero
CPU faults and working IRQs. Existing independent A20/HMA/ROM, segment-protection,
BIOS extended-copy and invalid-hole tests remain enabled. No test disables A20
underneath a live DOS HMA kernel or writes to DOS-owned HMA.

The 540 KiB target is exceeded by 66,704 bytes. No commercial game was obtained
or tested; this result establishes the allocation target, not game compatibility.
No third-party MEM is bundled: the original MEMSIZE utility gives a direct,
reproducible measurement without another guest dependency.

### Delivery, source and browser checks

The [driver license audit](licenses.md#high-memory-desktop-driver-issue-132)
records exact provenance and Artistic redistribution conditions. The new five
same-site release/source/notice assets are intentional additions: **24 approved
site files**, including **14 DOS artifacts**. All six upstream input archives
are pinned. A clean desktop rebuild unpacks `desktop-source.tar`, supplies
those archives in `fixtures/freedos/`, runs
`python3 tools/prepare_desktop.py --update-manifest`, and compares all fourteen
published artifact hashes with `desktop-publication.json`. It matched exactly.
Normal preparation (without the flag) also passes deterministic manifest checks.
Corrupting the published HimemX source and adding an unapproved sentinel asset
were each rejected by the strict audit, then restored and rechecked successfully.

The sparse template remains 655,360 bytes (0.4883% of the logical HDI); no full
128 MiB image is published or added to source control. The new guest files consume
11 extra 4 KiB disk clusters. Focused Chromium measurements on macOS arm64,
Playwright 1.56.1, two workers: mount 113/93 ms and natural cold boot 4,938/4,913 ms
at `/` and `/pc98-rust/`. Wasm high-water memory was 14,090,240 bytes and resident
HDD pages 458,752 bytes, unchanged from Issue #130. Sampled renderer peak RSS was
186,896/185,664 KiB; total browser peak RSS 408,048/405,824 KiB. Process sampling
has the limitations described above; native validation was also running locally.

Actual startup HTTP bodies totalled 1,241,926 bytes at each root: HTML 4,250,
CSS 3,356, JS 22,165, Wasm 552,196, manifest 4,599 and template 655,360 bytes.
Release/source/notice archives are not downloaded at startup. These are local
uncompressed HTTP observations, not deployed-site timing or compression claims.

The desktop browser tests reuse existing cold/reset, restored-HDD and full
export/reimport flows for high-memory checks. Only one new lightweight baseline
scenario per root is added; no additional full 128 MiB export is performed.
CI prepares the audited desktop before the new native test. The complete suite
retains floppy hot insert, both demos, boot-source separation, dirty-data warnings,
profile retention, IDE large-HDI writes and exports. Source links are downloaded
and verified from both roots, including all five new driver artifacts.


### Final pre-merge validation (2026-11-05)

- All documented acceptance-workflow fixture preparations, original media/font
  hash checks and Python fetch tests passed; combined preparation 18.48 s.
  The expanded fetch suite separately passed all **10 tests** in 0.006 s.
- `cargo test --locked`: **556 passed, zero failures**, 327.63 s elapsed.
  This includes baseline/high allocator controls, HMA execution, XMS recovery,
  historical DOS/VM controls and all existing CPU/device/disk regressions.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` passed.
  The first Clippy run found a redundant comparison in the new test; it was
  removed before the final successful check (0.66 s for Clippy).
- Release Wasm built successfully (552,196 bytes; initial compile 1.42 s).
  `npm test -- --workers=2` performed the final media/font checks, deterministic
  desktop preparation, release build and **24-asset publication audit**, then
  ran the full two-root Chromium suite: **231 passed, 3 intentional existing
  skips, zero failures** of 234 scenarios, 6.3 minutes. Total build-through-suite
  feedback elapsed was 378.49 s. Local native/browser runs partially overlapped;
  these are elapsed times, not summed runner computation or CI queue timings.
- Focused desktop Chromium validation: **30 passed** at both roots in 38.5 s.
  The focused native allocator/XMS control passed cold/reset for both baseline
  and high configurations, including repeated measurements, in 23.87 s.
- Clean helper-source desktop rebuild: all **14 artifacts identical**, 0.19 s.
  Normal deterministic regeneration, corrupted-source/unapproved-asset rejection,
  restored strict publication audit, changed Markdown file links and
  `git diff --check` passed.

Intentionally unchanged historical low-DOS SHA-256 values:

- `minimal.hdm`: `015faaa0dfb4c23f7a8aa4b74bbad2d460afc55c64517c5b4ea7330d4a8f8d1e`
- `shell.hdi`: `fb44e34131bfa2ca56ffb11ad004446c096718233391f2828b4d488114b40bcb`

All other historical manifests and fixture builders are unchanged and their
preparation checks passed. No private media, commercial game, proprietary ROM or
font, manual gameplay, published-site or post-merge check was required or used.
