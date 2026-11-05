# Large local HDI media (#127)

The former 32 MiB rejection was an implementation guard around contiguous upload,
insert and export copies. It was not an emulated PC-9821Ap hardware limit. The
browser now imports a user-selected local HDI in 1 MiB chunks, into 64 KiB pages.
Zero pages consume metadata only. An occupied page is stored once in Wasm; native
`Machine::insert` instead takes ownership of its existing `Vec` without copying.
There is no network upload, bundled HDD, automatic formatting, or create-disk UI.

## Distinct bounds

| Layer | Implemented boundary |
| --- | --- |
| HDI container | Little-endian 32-bit offset, payload length, sector size and CHS fields at bytes 8–31. Offset must be at least 32. Exact file length must equal offset + payload, calculated in 64 bits. Padding/reserved header bytes are retained. Payload must equal C × H × S × 512, without truncation, trailing bytes or inferred geometry. |
| Portable host/browser extent | File length at most `u32::MAX` (strictly below 4 GiB), because the production wasm32 import/export ABI uses 32-bit byte addresses and lengths. This also excludes a nominal 4 GiB payload plus its header. It is a supported implementation boundary, not a claim that all HDI variants have this limit. |
| Emulated device | 512-byte physical sectors; 1–65535 cylinders, 1–16 heads, 1–63 sectors/track. IDE CHS sectors are one-based; BIOS CHS sectors are zero-based. ATA LBA is 28-bit. The HDI length field/portable file extent is tighter than 28-bit ATA capacity. IDENTIFY words 60/61 report the complete sector count; geometry is reported unchanged. |
| Synthetic BIOS | INT 1Bh uses the header geometry, 16-bit cylinder, or a full 32-bit LBA from DX:CX, bounded before translation to IDE. BX byte count is nonzero, sector aligned and 16-bit (at most 65024 bytes/request). Guest RAM validation remains in place, including protected/VM86 calls. IDE IRQ/INT 11h semantics are unchanged. |
| Guest partition/filesystem | Determined by guest DOS, its boot loader, partition table, FAT variant and cluster size. HDI capacity is not a promise that every historical DOS installer or partition can address that size. No guest-specific media hooks are used. |
| Browser resources | Available Wasm linear memory, address space, browser file/Blob/download limits and OS memory. Occupied pages require their actual bytes; sparse capacity is inexpensive. Replacement may temporarily require both old and new occupied pages. Allocation failure rejects the candidate without replacing the mounted disk. Memory pages retained by the Wasm runtime may keep the linear-memory high-water mark after eject. |

512 MiB and 1 GiB guest I/O are tested with explicit geometries 2048×16×32 and
4096×16×32. Larger geometries within the bounds are accepted, but are not claimed
as tested DOS installations. Keep original media and export modified images.
Unmounting/reloading the page does not persist an unsaved disk automatically.

## Import, execution, export and failures

The browser reads the header and validates it against the actual `File.size`
before allocating disk pages. It then reads one bounded slice at a time. Rust
stages pages outside the mounted device and commits only after exact completion
and revalidation of the imported header. Short reads, invalid headers, allocation
errors and cancelled imports release staging and leave existing media/writes and
boot selection intact. UI media/profile actions are disabled during the operation;
execution resumes after a failure. Core imports also enforce chunk alignment and
extent. Reset moves the disk backing without cloning and retains dirty pages.
PC-9801VM still cannot attach IDE, and profile switching retains existing media.

After import every page is resident or known zero. IDE PIO and BIOS transfers do
not depend on asynchronous callbacks or host file latency. Allocating a previously
zero page on a guest write is fallible; failure returns an IDE error, without
partially changing a sector. An unaligned header can put a sector across two pages;
both allocations precede any data modification. Out-of-range requests are rejected
before writes. The emulator does not parse guest partitions or filesystems.

Export pauses execution to snapshot changed pages. A Blob is composed of original
local File slices and snapshots of dirty pages; unchanged bytes never traverse
Wasm again. Changed-page snapshots are bounded to 64 KiB, and long dirty-page lists
yield periodically to the browser scheduler. There is no whole-image transfer
buffer, JavaScript number array, base64 string, or second complete Wasm image in
the production download path. If every page has changed, the browser must retain
the changed bytes for its download snapshot; available browser/OS resources still
matter. Blob/download creation errors are displayed and the disk remains mounted
for retry. Browser/process termination cannot be recovered by application code.

The original File stays referenced until replacement/eject. Failed replacement
never discards it. Export preserves the original full extent, reserved header,
partition table, untouched nonzero sectors, zero sectors and guest writes. It is
not a filesystem reconstruction. `read_bytes` and Wasm `hdi_read_chunk` provide
bounded inspection; the legacy JavaScript number-array inspection helper refuses
large disks and directs callers to `exportBlob`. Tests stream full-image hashes in
Node, never large arrays through Playwright RPC or generated large artifacts.

## Reproducible evidence

Run the README preparation commands, including `prepare_large_hdi_fixture.py`
and, after the pinned FreeDOS binary/source preparation,
`prepare_large_dos_fixture.py`. Large files are generated locally as sparse files
and removed after browser tests. The checked-in files are original source, small
manifests and tests. GPL outputs remain in ignored `fixtures/freedos/`, alongside
matching complete source archives and notices, excluded from the approved Pages
asset tree.

* Native `large_hdi`: malformed/truncated/oversized headers, non-512 sectors,
  unsupported geometry, overflow-sized extent, chunk bounds, incomplete import,
  unaligned header/sector crossing, replacement safety, 512 MiB/1 GiB actual guest
  IDENTIFY/BIOS reporting, CHS/LBA low/middle/last reads and writes, two-sector PIO,
  IRQ/error/residual, maximal 32-bit BIOS LBA overflow rejection, reset and eject. Existing storage suites retain the broader
  small-disk VERIFY, IRQ, floppy routing and protected/VM86 regression matrix.
* Production Chromium `large-hdi.spec.js`: 512 MiB attach, failed staged local-file read recovery, guest writes beyond
  32 MiB, last-sector access, reset, failed replacement, download, streaming
  byte-for-byte expected-image SHA-256 and actual reimport/persistence at both
  `/` and `/pc98-rust/`. One 1 GiB attach/guest probe and one occupied 512 MiB
  resource scenario run at `/`, avoiding redundant expensive combinations.
* The legal unmodified FreeDOS(98) kernel/FreeCOM and HDD FAT16 boot source run on
  a legitimate 512 MiB FAT16 partition (32742 clusters, 16 KiB clusters). An
  original blank file occupies low clusters so DOS allocates `NEW.TXT` at cluster
  8193, LBA 262914 (>128 MiB). DOS reads `HIGH.TXT`, creates and reads `NEW.TXT`,
  returns to its shell and runs the original DOS PROBE, cold, reset and after
  export/reimport. The latter runs read the prior `NEW.TXT` before rewriting it. This
  demonstrates the selected guest's FAT16 support without changing OS or adding
  an emulator special case. It does not validate every DOS variant/installer.

## Measurements (2026-11-05)

Local Apple arm64, macOS 27.2, 64 GiB RAM, Node 26.12.1, Playwright 1.56.1,
Chromium 141.0.7390.37. Native compiler: Homebrew Rust 1.94.0; production Wasm:
rustup Rust 1.99.0, release/LTO/opt-level 2. One-worker focused measurements use
actual local file selection and completed browser downloads, including writing the
512 MiB output to disk. They are observations, not cross-platform timing gates.

| Scenario | Import | Export/download | Wasm high water | Sampled browser RSS on import | Sampled browser RSS on export |
| --- | ---: | ---: | ---: | ---: | ---: |
| Sparse 512 MiB, `/` | 0.867 s | 0.665 s | 13.5 MiB | 398.0 MiB | 399.8 MiB |
| Sparse 512 MiB, `/pc98-rust/` | 0.866 s | 0.585 s | 13.5 MiB | 399.5 MiB | 398.8 MiB |
| Occupied 512 MiB (~510 MiB nonzero), `/` | 0.871 s | 0.621 s | 522.8 MiB | 881.1 MiB | 902.0 MiB |

RSS is sampled every 100 ms across processes reported by Chromium's SystemInfo,
including the browser/download process and renderers; summed RSS may count shared
pages twice and may miss short-lived peaks. It excludes OS filesystem cache and
the Node test runner. Browser baseline for the sparse runs was 331.6/336.7 MiB;
renderer-only import/export peaks were 192.4/178.8 MiB at `/`, 193.8/177.9 MiB
at the Pages root, and 662.5/677.5 MiB for occupied media. Occupied disk backing
was 534974464 bytes, and Wasm memory remained exactly 548208640 bytes across
export. This distinguishes native browser costs from the disk's single Wasm
copy; the application makes no additional full-image JS/Wasm export buffer.

Sparse 512 MiB reimport took 0.860/0.861 s, with sampled browser RSS peaks of
456.4/454.3 MiB. The 1 GiB import plus actual guest probe took 0.878 s. The
focused large-media file ran five scenarios in 15.5 s, with three intentional
Pages duplicates skipped. That file runtime includes occupied-media generation,
streaming full-image equality checks, guest execution and cleanup;
no giant download is retained as an artifact. All runs used local test-generated
or checksum-pinned legal media, never private images.

## Final validation

* All README fixture/source/license preparation commands passed (the initial
  preparation batch took 12.19 s); both new preparation scripts subsequently
  passed against their final pinned manifests. The fixture-fetch Python suite
  passed all seven tests.
* `cargo test --locked`: **550 passed**, zero failed/ignored across 37 targets,
  302.525 s wall time. The four new native large-disk tests took 9.94 s.
* `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`: passed.
  The Wasm-target library also passed Clippy with the rustup toolchain selected
  explicitly in PATH/RUSTC.
* `npm test -- --workers=4`: production release-Wasm build and the approved
  publication-tree audit passed (exactly ten public assets), then **199 Chromium
  tests passed**, zero failed, **three deliberate duplicate scenarios skipped**,
  at both serving roots. Total build/test wall time was 181.799 s (Playwright:
  3.0 minutes). Only the extra 1 GiB, FAT16 DOS and occupied-media cases are
  omitted at the Pages root; the full 512 MiB round-trip runs at both roots.
* The final one-worker large-media measurement run passed all five selected
  scenarios in 15.5 s. `git diff --check` passed. There are no required manual
  checks or post-merge verification steps.

Development failures were resolved before the final passing runs. The first
browser export attempt interpreted Wasm's `u32::MAX` dirty-page end marker as a
positive JavaScript value instead of signed `-1`; explicit unsigned conversions
fixed that and all byte pointers/extents. The initial native probe used a
non-teletype INT 18h display service and an IRQ assertion with nIEN still set;
the original guest probe/output and assertion were corrected. An intermediate
whole Chromium run (two workers, 7.2 minutes) finished with 194 passed, five
failed and three skipped: four failures exposed an unnecessary mouse release
when pausing Floppy 2 insertion, fixed by limiting suspension to HDI; one export
hash mismatch came from regenerating the probe fixture during that run. Fixtures
were frozen and the complete suite rerun successfully against the final build.
An initial Wasm check/extra lint invocation selected Homebrew Rust without its
Wasm standard library; using the rustup compiler and matching toolchain PATH
resolved it. No failing check or required validation remains pending.

## Stable-toolchain review revision (2026-11-05)

Reproduced the review's `clippy::chunks_exact_to_as_chunks` failure in the
FAT16 root-directory assertion with Rust/Clippy 1.99.0. Replaced
`chunks_exact(32)` with `as_chunks::<32>().0.iter()`, preserving the lookup of
complete 32-byte directory entries. The following acceptance rerun used the
rustup stable toolchain's bin directory first in PATH for every Rust/build command,
matching CI's stable-toolchain selection rather than the host's Homebrew Clippy
1.94.0. These are local macOS arm64 results, not a new GitHub Actions run.

* All fixture/source/license preparation and Python checks from
  `.github/workflows/emulator.yml` passed in 10.06 s.
* `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` passed.
* `cargo test --locked`: **554 passed**, zero failed/ignored, 287.15 s wall time.
  All four `large_hdi` tests passed in 9.07 s, including the affected FreeDOS
  FAT16 assertion above 128 MiB and the 512 MiB/1 GiB guest I/O/reset probe.
* `npm ci` and `npx playwright install chromium` passed.
  `npm test -- --workers=4` passed the production release-Wasm build, the
  ten-approved-asset publication audit, and **201 Chromium tests** at `/` and
  `/pc98-rust/`, with zero failures and the same three intentional Pages
  duplicates skipped. Total build/test wall time was 168.54 s (Playwright: 2.8
  minutes), including the 512 MiB export/reimport at both roots.
