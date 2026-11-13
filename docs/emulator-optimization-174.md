# Planar scanout and fetch optimization

The synthetic changing-snapshot controls use 24–40% less raster/copy time with byte expansion
and packed RGBA output. A separate fixed-work game control uses 1.3% less active
execution time by the median. These are different workloads and their percentages are
not additive. This implements [Issue #174](https://github.com/takahirox/pc98-rust/issues/174)
and makes a small contribution to the CPU work in
[Issue #172](https://github.com/takahirox/pc98-rust/issues/172), which remains open.

## Selected changes

Planar scanout expands each plane byte into eight palette-index bytes through an
original 2 KiB lookup table. It combines the planes once per eight pixels, then
composites the existing text dots. Preparing four-byte RGBA palette entries once
per rasterized frame also replaces separate RGB and alpha stores with one pixel
copy. Palette changes, display pages, wrapping, 40-column text, 200-line doubling,
VM/Ap planes, blink and PEGC retain their existing interpretation. The existing
snapshot comparison still reuses identical frames.

Byte expansion tables are also used in
[NP2kai's graphics renderer](https://github.com/AZO234/NP2kai/blob/253ef651c9d5d8f6d11f9cc7326b4f05ce5c9ce4/vram/makegrph.c).
The Rust table and implementation here are original; no emulator dependency or
source import is added.

The guarded JIT fetch path now accounts once per touched cache line when caching
is enabled, or directly charges the original four-byte beats when disabled.
A supported instruction is at most seven bytes and touches at most two lines.
Repeated fetch hits within one line cannot alter the round-robin replacement
state. Ordered fills, uncached clocks, the final fetch word and final byte match
the original beat path. Every instruction still validates its code bytes and
retains its IRQ, work/deadline and device boundaries. Operand reads and writes
continue through the existing bus.

This applies the state specialization principle described in
[QEMU's translator documentation](https://www.qemu.org/docs/master/devel/tcg.html#cpu-state-optimisations)
to the existing bounded RAM fetch path. Generated RAM runs, direct successor
links and broader flag deferral remain separate experiments under #172.

The 574,356-byte Wasm is 2,314 bytes larger than the 572,042-byte baseline
(0.40%). Baseline main is `01ba6eb4d25aae3e63e84941c078009a4ae6d9f5`.
Baseline Wasm SHA-256 is
`6d9d2c74eb9bb56278dd041c224a3535ce016fda3bd61077d16c5838531aea57`;
selected Wasm SHA-256 is
`7491fb290f287e5eba5295cc2ca5a4ac5f19f0896bca6c3a679230557ef3d8ff`.
Artifacts use Rust 1.99.0, release opt-level 2/LTO, Wasm SIMD and Binaryen 132
`-O3 -g` through the existing build script. The generated-block compiler
`web/jit.js` is unchanged.

The first GitHub acceptance run used Rust 1.100.0 and failed Clippy's
`needless_range_loop` check on two existing sound-timer and VM86 test-helper loops.
Both now use mutable iterators, retaining warning denial and every assertion.
The final-artifact scanout and fixed-work controls were repeated after
this cleanup, and all intermediate/final game states match the earlier artifact.
The production matrix uses that earlier artifact, explicitly identified below.
The same lint also required iterator cleanup in existing FDC, paging, VM86,
CLTS and PIT integration-test helpers, with unchanged assertions and no further
runtime change. All-target Clippy passes locally using the explicit Rust 1.100.0
Clippy binary; the shell's `cargo clippy` otherwise selects Homebrew's older tool.

## Scanout controls

Stock headless Chrome 157.0.8103.52 on Apple M4 Max, arm64 Darwin 27.2.0 and Node
26.12.1 runs both frozen artifacts in one dedicated Worker, without a debugger or
CPU profiler. Original synthetic snapshots cover both profiles and 32 mode/flag
combinations at three blink times. The measured workloads warm 256 frames per
artifact, alternate artifact order on every frame and reverse it between trials.
Each timing includes copying the snapshot into Wasm and copying the pixels into
a reusable output buffer, matching production buffer reuse. One displayed VRAM
byte changes per changing snapshot; the static control preserves every byte.

Three trials use 144 frames each for changing snapshots and 1,440 each for static
snapshots. The primary and reversed-artifact controls each compare all output
pixels on 6,240 frames. Every comparison matches. Times below are medians in
microseconds per frame, not visible presentation latency:

| Workload | Baseline | Selected | Reduction | Reversed-artifact reduction |
| --- | ---: | ---: | ---: | ---: |
| VM planar | 1,043.8 | 641.0 | 38.6% | 38.4% |
| Ap planar | 1,056.3 | 638.9 | 39.5% | 41.5% |
| Ap planar with 40-column text | 383.3 | 272.9 | 28.8% | 22.5% |
| Ap identical snapshot | 159.7 | 160.8 | −0.7% | 0.8% |
| Ap PEGC | 736.1 | 559.0 | 24.1% | 22.7% |

The static differences change sign and are within trial variation; they do not
establish an identical-frame improvement or regression. PEGC benefits from packed
palette output rather than the planar table. These controls measure scanout and
copies, not whole-emulator speed or display/GPU throughput.

Run the opt-in control after saving baseline and candidate release artifacts
built with the same toolchain:

```sh
node tools/measure_scanout.mjs --baseline=/tmp/pc98-baseline.wasm --candidate=/tmp/pc98-candidate.wasm --output=/tmp/pc98-scanout.json
node tools/measure_scanout.mjs --baseline=/tmp/pc98-candidate.wasm --candidate=/tmp/pc98-baseline.wasm --output=/tmp/pc98-scanout-reversed.json
```

The tool needs installed Google Chrome and the existing Playwright dependency.
It serves only the supplied artifacts and original synthetic data on owned
loopback HTTP. Reports must stay outside the checkout; existing output files are
not overwritten. The control is not added to normal CI.

## Fixed guest work

Three fresh pairs use the installed base game's deterministic 24.0002795 guest
second prologue and the boot/input sequence from
[the existing game harness](../tools/measure_doukyuusei2.mjs). Two independent cores
alternate 10 ms slices in one owned Chrome Worker. Compilation waits occur
outside the measured intervals, with pending compilation settled between slices.
The measurement includes CPU/device execution and PCM hashing, excluding scanout
and AudioWorklet scheduling. No build, other benchmark or profiler competes with
the measured intervals.

| Control | Baseline active ms | Selected active ms |
| --- | ---: | ---: |
| 1 | 5,503.1 | 5,445.8 |
| 2 | 5,547.7 | 5,481.8 |
| 3 | 5,515.6 | 5,433.5 |
| Median | 5,515.6 | 5,445.8 |

The median reduction is 1.3%. All six controls preserve 118,102,981 completed
instructions, 1,584,018,440 CPU clocks, 131 disk reads, 1,464 FM key edges,
934 sound IRQ edges and 1,152,013 PCM samples. Each translates 93,107,752
instructions. CPU, final-frame and complete PCM-bit fingerprints match, including
at the intermediate eight-guest-second boundaries. The CPU fingerprint covers
the low GPR bank, FLAGS, IP and CS. This is modest execution
headroom; it does not establish a 1.3% reduction in visible scene-transition time.

The [sanitized numeric report](emulator-optimization-174-results.json) retains
artifact/harness hashes, timing trials and aggregate fingerprints. Private media,
installed disks, raw game reports, guest code, pixels and PCM remain outside
version control and publication.

## Rejected CPU trials

The separate baseline attribution in
[Issue #172](https://github.com/takahirox/pc98-rust/issues/172)
identified preparation as a visible sampled host function, including inlined
work. Profiled elapsed times are excluded from the performance controls.

| Trial | Screening result and decision |
| --- | --- |
| Masked 64-bit code-byte comparison | A compact variant was 7.1% slower in a paired game screen. Retain the original byte-span comparison. |
| Dedicated register retirement loop | 0.06% lower active time, effectively neutral. Removed. |
| Predecoded address timing | 0.35% lower, insufficient evidence. Removed. |
| Address predecoding with continuation-state reuse | Roughly 0.9–1.2% lower in single screens, without a demonstrated advantage over the much smaller fetch specialization. Removed. |

These screens select candidates; single-pair percentages are not repeatability
evidence or additive gains. No code-page generation tracking, speculative RAM
stores, bulk REP conversion or general interpreter checkpoint change is retained.

## Production playback

These captures use the pre-cleanup 574,197-byte candidate with SHA-256
`fda47094aad0e5ab2161ee8fdcfbcb6c494c35867a58a9c2d9889b50a3ad0c87`,
before the two iterator changes required by Rust 1.100 Clippy. They characterize
that artifact's production behavior; they are not final-binary playback captures.
The final artifact is covered by the repeated scanout/game controls and acceptance
checks. The numeric report records both artifact identities.

Ten additional controls use the cached arm64 Playwright 1.56.1 Linux image,
Chromium 141 and Node 22.20.0. Each starts from normal audio-enabled boot with
JIT enabled, the same installed disk and 48/24 wall-paced clicks. Runtime and
measurement identities are frozen; no CDP, debugger, profiler, guest fast-forward
or patch readback runs. The whole browser container uses two allowed CPUs,
2 GiB RAM and a 4 ms CFS period. Unrestricted controls are followed by a 0.5-CPU
quota, repeated twice for each artifact. A final baseline pair reverses artifact
order to check variation. Every capture/calibration is valid, all owned containers
are removed, and no emulator/render fault, overflow, drop, sequence or checksum
error occurs.

The first input-window ranges across the two primary repeats are:

| Artifact and budget | Guest s / wall s | Presented fps | Missing device frames | Mean raster ms |
| --- | ---: | ---: | ---: | ---: |
| Baseline unrestricted | 1.000 | 60.00 | 0 | 0.297–0.351 |
| Selected unrestricted | 1.000 | 59.96–59.97 | 0 | 0.248–0.274 |
| Baseline 0.5 CPU | 0.464–0.480 | 9.50–12.84 | 745,943–771,993 | 3.634–4.916 |
| Selected 0.5 CPU | 0.399–0.456 | 7.35–10.06 | 783,928–871,865 | 2.863–3.471 |

The unrestricted controls preserve real-time production and approximately 60 fps.
The failing budget still fails. Calibration Worker slowdowns range from 2.12–2.97x
for the primary baseline controls and 2.74–3.90x for the selected controls, despite
the same configured quota. These are observed probe ratios, not normalization
factors for game execution. The baseline holdout observes 3.62x/3.87x, 0.496
guest/wall, 7.67 fps and 716,663 missing device frames in that window. Its healthy
control remains real-time at 59.97 fps with zero missing frames.

Input progress also differs: both artifacts have constrained traces ending at
973 HDD reads, while other traces reach 1,030. The holdout's first window reads
105 sectors rather than the primary constrained windows' 131. Phase names mark
attempted inputs, not verified identical game scenes. Parallel Worker elapsed
times include descheduling and cannot be added as CPU shares. These controls do
not establish a causal scene-speed improvement or regression between artifacts;
the fixed-work and synthetic controls establish the retained computational gains.
The missing-PCM/low-end throughput problem in
[Issue #146](https://github.com/takahirox/pc98-rust/issues/146) and larger CPU work
in #172 remain open. No claim of universal low-end 60 fps follows.

The reproduction command is unchanged apart from skipping missing Git worktree
records during path validation:

```sh
node tools/measure_cpu_budget.mjs --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-evidence/matrix.json --budgets=unlimited,0.5 --repeat=2 --clicks=24
```

Use a separate output for each frozen artifact, one control at a time, without
builds, tests or other emulators competing. The
[CPU-budget documentation](cpu-budget-evaluation.md) describes the resource and
counter units, calibration and private evidence boundary.

## Regression validation

The 57 fixture preparation/check commands from the acceptance workflow pass,
including the legal FreeDOS fixtures and their matching source/notice checks.
`cargo test --locked` with test opt-level 1, debug assertions and overflow checks
passes 634 tests; Clippy with warnings denied and Rust formatting pass. The
release build/publication audit passes with 28 approved assets. All 56 Node tests
pass, including 27 actual release-Wasm JIT tests. The four CPU-budget helper tests
also pass after the measurement-path fix.

The pinned host browser was absent. Its download passed archive CRC checks, but
host extraction stalled; the installed alternate Node runtime also could not
start because of a missing shared library. Browser acceptance therefore runs in
the already cached Playwright 1.56.1 Linux image with Node 22 and Chromium 141,
network disabled and only the required test assets mounted read-only. No project
dependency or browser-test assertion changes are made for this environment.

Both isolated audio cases pass with zero missing samples. The functional run
passes 257 cases with four existing Pages skips; its one occupied-512-MiB failure
coincides with a recorded container OOM kill under four workers and a 6 GiB
limit. That case passes alone with the same limit and all original assertions,
including the full-image equality and Wasm high-water bounds. All 258 required
functional cases therefore pass across the full run and focused rerun.

New native coverage checks every planar pixel against a scalar bit reference
through representative pages, wraps, doubling and column modes, and compares
specialized fetch timing/cache state with the original beat path through cache
conflicts and invalidation. The actual-Wasm memory regression covers aliases,
live address dependencies and every committed prefix with caching on and off.

The measurement tool now ignores only missing paths in Git's retained worktree
records and reads names with NUL delimiters. A Git fixture regression retains
Unicode worktree protection alongside a deleted record; direct CLI checks also
reject media and report destinations inside an existing checkout. The captured
wrapper hash and the hash after this path-only hardening are both recorded; the
frozen playback/calibration scripts and runtime assets are unchanged. Three invalid scanout CLI cases fail before browser startup.
The change leaves Git's stale worktree metadata untouched.

After the iterator cleanup, the native library, sound, Ap, music and PIC focused
checks pass again, as do the release build/publication audit and all 56 Node
checks. The final artifact repeats both complete scanout controls and all three
fixed-work pairs with matching output/state fingerprints.

Required post-merge verification: None.
