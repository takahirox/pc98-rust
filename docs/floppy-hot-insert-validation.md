# Installed floppy drives and live DOS media (Issue #126)

Both synthetic profiles have two installed 2HD floppy units. Media insertion is
separate from equipment configuration. For the tested floppy-only FreeDOS boot,
DOS assigns A: to DA/UA 90h and B: to 91h, including when FDD2 is empty at startup.
An IDE boot retains the guest's PC-98 drive-letter policy; this change does not
select boot media or assign DOS letters in the host.

## Diagnosis and licensed evidence

The existing checksum-pinned `minimal.hdm` already uses the exact distribution
reported in the Issue: FreeDOS(98) test-20220120-cherrypick, kernel build 2043
compiled January 20, 2022, and FreeCOM 0.85a_DBCS compiled January 17, 2022.
There is no guest-version substitution or modified kernel/shell.
`tools/prepare_freedos.py` verifies the distributions and complete corresponding
source archives, preserving GPL notices alongside the generated boot images.

The inspected guest source is
[lpproj/fdkernel at 679eda7](https://github.com/lpproj/fdkernel/tree/679eda7eeeac6cc68bad4d3ff9b055e672529731/nec98):

- `kernel/initdisk.c`, `BIOS_nfdrives_nec98` (lines 1768–1809), reads the
  DISK_EQUIP word at 0000:055Ch. Low bits identify 2HD units and produce DA/UA
  values 90h, 91h, etc. `make_floppy_ddts` (1984 onward) constructs DOS device
  entries and publishes the unit list at 0060:006Ch. The equivalent logic is also
  present in `kernel/initdsk2.c`. This is an installed-unit list, not a media list.
- The same initialization queries INT 1Bh AH=C4h/84h to classify drive modes.
  Those queries do not dynamically create a missing DOS drive after startup.
- `kernel/dsk.c`, `diskchange` (340–380), examines the saved per-unit FDC ST0 at
  0000:0564h + unit*8 and calls `fl_sense`. A successful sense plus saved ST0
  below C0h permits the cached medium; a ready transition requires revalidation.
  `mediachk` and `getbpb` then implement the guest's BPB/serial/cache checks.
  `kernel/floppy.asm`, `FL_SENSE` (41–51), calls INT 1Bh AH=04h and converts
  CF/AH to the DOS driver's sense result.

This diagnosis follows the independently licensed guest interface, without
consulting or copying an external emulator implementation or proprietary ROM.

Against the original worktree code, **both VM and Ap regressions fail** after
post-boot D88 insertion: equipment=01h, DOS unit list `[90h, 00h]`, genuine shell
input `b:`, an error, and return to A:. No FDD2 DOS entry exists. A temporary
baseline control with the same disks mounted before reset passes `B:`, `DIR`, and
`TYPE SECOND.TXT` on both profiles, with observed INT 1Bh FDD2 reads. This
reproduces the reported reset distinction using only cleared inputs.

Changing only equipment to 03h fixes enumeration and permits the first file read,
but an independent swap regression still fails: DIR displays SECOND.TXT after
SWAPPED.TXT media replaces it. Previously the synthetic BIOS neither recorded
media ready transitions nor maintained its saved FDC result area. The zero/old
status allowed the guest to retain cached filesystem data. Thus 055Ch is the
specific cause of the missing B: entry; media-status bookkeeping is an additional
required correction for reliable swaps.

## Implementation and regression boundaries

`firmware::initialize` advertises both installed units independently of mounted
media and initializes their saved status as ready transitions. `Bus::mount_floppy`
records ST0.IC=11 for the selected unit's media transition in the synthetic BIOS
work area, leaving the existing PIO termination path intact. Each completed BIOS
transfer saves the seven actual FDC result bytes for that unit, consuming the
transition after the normal guest media check. This is synthetic firmware status
bookkeeping, not a new raw-controller interrupt/polling implementation.

Actual BIOS reads of an empty unit still return CF=1/AH=80h; direct FDC transfers
also fail without copying payload. Installed equipment stays 03h after eject.
The existing INT 1Bh device, media, CHRN and readiness checks remain in force.
No host DOS handling, forced prompt, substituted A: disk or insertion reset is
introduced. Boot selection, IDE equipment and HDD services retain their paths.

`tools/prepare_floppy_data.py` independently authors a nonboot FAT12 BPB, FATs,
root directory, file data and D88 headers: 77 cylinders, two heads, eight
1024-byte sectors. SECOND.TXT contains `ORIGINAL FDD2 FILE CONTENTS`; the
replacement SWAPPED.TXT contains `REPLACEMENT FDD2 FILE CONTENTS`. Raw and D88
products, source hashes and MIT notice are checked by
[`floppy-data-manifest.json`](../fixtures/floppy-data-manifest.json). No DOS boot
code, filesystem slack, game asset or other acquired content enters these data
disks. Generated test media stay outside the publication tree.

Coverage is deliberately split across layers:

- Native BIOS tests exhaust both profiles and all four initial media masks,
  including no-media CF/AH and unchanged guarded read buffers. A selected-unit
  transition test checks status, no-media failure and replacement bytes without
  disturbing the other drive's status. Existing FDC PIO tests retain active
  transfer/eject/swap coverage.
- `tests/floppy_hot_insert.rs` boots the unchanged GPL shell on **both profiles**,
  starts with only FDD1 mounted, inserts FDD2 D88 without reset, executes actual
  keyboard `B:`, `DIR` and `TYPE`, and observes real INT 1Bh reads of DA/UA 91h.
  It repeats TYPE (allowing legitimate DOS cache reuse), swaps directly to raw
  replacement media, checks the replacement directory/content, ejects/reinserts
  D88, and repeats the mounted-at-reset control. At the end an empty-drive DIR
  reaches DOS's genuine critical-error prompt rather than a cached directory.
  Full floppy bytes, zero guest writes and zero CPU faults are checked.
- `tests/browser/floppy-hot-insert.spec.js` covers the **production Wasm** Ap
  guest at **both `/` and `/pc98-rust/`** through the normal file picker, explicit
  Floppy 2 selector and physical Chromium keyboard events. It verifies unchanged
  guest state on insertion, the correct UI labels/boot source, repeated file
  reads, direct D88/raw swap, eject/reinsert, full exports of both data disks and
  FDD1, the reset control and the guest no-media error. Native VM coverage avoids
  multiplying the same browser/path combinations. Existing routing, profile,
  keyboard, HDD, boot and reset acceptance remains in the full suite.

The native live sequence has a 12-million-slot bound and the reset control an
8-million-slot bound; browser typing uses the established 50,000/10,000-slot
press/release cadence to avoid overflowing the guest input path. The focused
browser sequence uses 10,920,000 slots before reset and passes both projects in
about four seconds total on this development host. These are local observations,
not CI timing targets. No external game or physical hardware test is required.

The first full browser run passed 194/196 cases and exposed an existing
`userfont.asm` probe assumption at both roots: the newly correct DOS FDD2
assignment line occupied text cells over the blue graphics glyph, so a boot-text
pixel covered its expected color. The original MIT probe now blanks only those
two text cells before drawing. The original FONTLOAD control and all BIOS, CG,
text/graphics pixel, replacement and reset assertions remain intact. This is
fixture isolation from guest boot output, not a rendering or DOS workaround.

## Validation

On Darwin arm64, the following checks passed:

- All 51 Python preparation/check commands from the README, including seven
  fixture-fetch unit tests, pinned GPL distributions/complete source archives,
  original offline probes, fonts and their notices. The new
  `python3 tools/prepare_floppy_data.py` also passes its checked source/image
  manifest; `prepare_font_fixture.py` was rerun after isolating its glyph area.
- `cargo test --locked`: **550 tests passed**, no failures or ignored tests.
  This includes the new VM/Ap guest sequences, firmware/FDC tests, IDE/HDD
  regressions and the unchanged user-font pixel assertions.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check`: passed.
- Production release Wasm build (`wasm32-unknown-unknown`, locked dependencies),
  original media/font reproducibility and the approved-assets audit: passed.
  The publication tree still contains exactly its ten approved assets; no DOS,
  source archive, test disk or acquired game is published.
- Focused real-Wasm FDD2 and corrected font tests: **2/2 passed each**, covering
  both roots. The first complete browser run's two font-probe failures and their
  fixture correction are described above.

- Final `npm test -- --workers=2`: **196/196 passed** at `/` and `/pc98-rust/`
  in 5.3 minutes, including both FDD2 scenarios, both corrected font scenarios
  and the complete existing routing/keyboard/IDE/HDD acceptance.

A separate native diagnostic also confirmed that after the genuine DOS no-media
critical error, reinsertion permits the next DIR from the returned shell without
reset. The permanent regression keeps broad media combinations at the native
layer and focused browser coverage at both paths.

No proprietary software, optional copyrighted game, live deployment or physical
hardware validation was used. No post-merge verification is required.
