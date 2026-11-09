# Local game performance — Issue #144

## Correction: Firefox automation did not measure normal execution

The historical Firefox measurements in this document used Playwright 1.56.1's
patched Firefox. Its injected debuggers observe Wasm and disable optimizing
compilation. Even launching that patched executable without a driver leaves its
frame/Worker agents active. Those rows describe forced debug execution, not
ordinary Firefox performance. The conclusions below that Firefox generally
cannot sustain real time are withdrawn. Chromium measurements are unaffected.

The [reproduction investigation](doukyuusei2-reproduction.md) compares stock
Firefox of the same version and changes only debugger observation flags in a
temporary copy of the patched browser. The old harness now requires explicit
`--debugTier=1` for that induced condition; use the normal-browser control for
ordinary playback. Historical counters and observations are retained below.

The [latest production matrix](#revised-production-measurements-review-follow-up)
covers the current CPU/PIC artifact in both browsers and renderers.

## Workload and media boundary

Use **ADV98 v0.1**, unchanged `ADV98.EXE`, author PokuG stdio.h, source
`11a9f1c59c6fd8169439d477d5276ee7c0f27566`, from the
[author release](https://github.com/yarufu/pc98/releases/tag/v0.1).
The executable SHA-256 is
`904f476516bf009b6d25d0a2738ddf649af05bcafac72cbb703c73d7159e5954`.
The release archive SHA-256 is
`9aa590f53b331ec04fa1a2352f7ed6190407756d1f552c9c0bf76f9a05eb2d3d`.
The downloaded source's root MIT license was inspected for this run. Retain
APP.LIC. Exclude the release's artwork, effects and music.

The [existing rights audit](audio-continuity.md#fixture-selection-rights-and-reproduction)
and [per-file manifest](../fixtures/music-manifest.json) cover every component:
original MIT replacement backgrounds, sprite, looping scenario and `play.mml`;
PMD 4.8s/MC built from published source with M.Kajihara's retained-copyright
free-use permission (the included Japanese `readme.txt` was inspected);
JWasm's separate build-tool license; FreeDOS/FreeCOM GPL sources and notices;
and the embedded Shinonome replacement font. No NEC BIOS/font/sound ROM is used.
Free download is not a redistribution license. Keep the complete prepared source
archives and notices locally; do not distribute a disk image by itself.

For this investigation the two workloads are the same playable adventure with
FM/SSG BGM, repeated CPU-decoded planar backgrounds/sprites and DOS disk reads,
on floppy and on IDE HDD. The second medium exercises a distinct I/O path,
not a second game. This already exercises the requested CPU/VRAM, scene and HDD
work; no additional title or unaudited mirror is needed. It does not represent
all games or a commercial target workload.

The benchmark images are outside the checkout. Existing explicitly licensed CI
fixtures remain unchanged and ignored. No new media, automatic game downloads,
CI jobs, uploaded profiles, audio or screenshots are introduced. The harness
reduces a tiny pixel probe inside the browser to detect the first changed
background; no captured pixels leave the page. Reports contain counters and
host function sample counts, never guest text, instructions, PCM or frames.

## Reproduction

Prepare the existing legal fixtures and production build using the
[README commands](../README.md#reproduce-the-checks). For just these workloads:

```sh
python3 tools/prepare_freedos.py
python3 tools/prepare_dos_apps.py
python3 tools/prepare_music_fixture.py
npm ci
npm run build
python3 tools/prepare_local_performance.py /tmp/pc98-issue144/media
node tools/measure_runtime.mjs --media=/tmp/pc98-issue144/media --output=/tmp/pc98-issue144/chromium.json --browser=chromium --repeat=3 --seconds=8 --profile=1 --revision=YOUR_REVISION
# Historical forced-debug control, not ordinary Firefox performance:
node tools/measure_runtime.mjs --media=/tmp/pc98-issue144/media --output=/tmp/pc98-issue144/firefox-debug.json --browser=firefox --debugTier=1 --repeat=3 --seconds=8 --revision=YOUR_REVISION
git check-ignore fixtures/dos-apps/adv98.zip fixtures/dos-apps/music-scenes.hdm fixtures/freedos/distribution.zip
git status --short
npm run check:site
```

Do not run the benchmark alongside compilation, regression suites or another
emulator. Use the same host, browser, build flags and media for both revisions.
The helper refuses media/report directories inside the checkout. Do not upload
local benchmark directories, even on failure. Existing publication allowlisting
checks the entire `web/` tree; neither these images nor reports belong there.

Media SHA-256:

| Image | SHA-256 |
| --- | --- |
| music-scenes.hdm | `59cbba363ce27e357001f5c57e29d73e0356ae5d599eef8fc431afea1e6ab728` |
| music-scenes.hdi | `ba799e6fea1910b9cf83a80ace054cd6e729dac01a2d2918de5609900fa1d3cc` |

The HDD helper repackages the exact scene files into the existing 64-cylinder,
8-head, 16-sector, 512-byte-sector FAT12/IDE layout with the original MIT IPL and
unmodified GPL DOS boot code. Only FDCONFIG.SYS's boot-device setup changes.
It makes no changes to the established CI fixture builders or manifests.

Both use **Ap DX2-66**, real-mode FreeDOS, 66 MHz CPU and the existing 8 MHz
device scheduler. VM remains V30-10; no clock/timing table changes are allowed.
Each browser page starts fresh, inserts a local disk, explicitly selects its
boot medium and resets. Only silent DOS/compiler/title setup is accelerated by
18 guest seconds. Sound then starts through the normal AudioWorklet, Return
starts the adventure, and the harness waits for music and 1.5 wall seconds of
settling. All measured production playback uses the real Worker scheduler.

Each observation has 32 polls separated by 250 ms, a Return at its midpoint,
and a bounded first-background-response probe. Reported wall duration includes
automation and transition waiting. Transition latency is first visible response,
not full-screen decode completion. A response not observed within one second is
reported as null; slow production is retained, not treated as passing or excluded.
Repeat three times, alternating inline/Worker
order. The bounded 0.1 ms slice histogram supplies p95/p99 upper bucket edges;
the last bucket means >=50 ms. It runs only with diagnostics enabled.

A separate manual capacity control starts from a fresh reset and exactly 18
guest seconds of setup, holds Return for 0.1 guest seconds, settles to two guest
seconds, then measures four guest seconds with another Return at two seconds.
No rendering, wall pacing or audio sink runs inside that measurement. Compare
exact clocks, instructions, PCM and IRQ/note counters as well as elapsed time.
Capacity above 1 does not prove production audio continuity; production near 1
does not imply spare execution capacity is unchanged.
`--capacityOnly=1` runs only this control for quick optimization screening; it
does not provide production playback or renderer evidence.

The optional Chromium host sample profile is collected **after** the production
window. Only aggregate function-name/sample counts are retained. Firefox uses
the same release Wasm and harness without a Chromium profiler. Browser-process
CPU seconds and summed resident bytes include the owned browser and its children;
shared pages may be counted more than once, so RSS is not unique memory usage.
Wasm linear-memory sizes are reported separately. These are local observations,
not a hardware acceptance threshold or a hosted-CI performance gate.

## Baseline diagnosis

The measured baseline is checkpoint `d2d318d450066d92a8e1d6754686cd2bd6825aa6`
(runtime inherited from PR #143), release Wasm SHA-256
`c71cfd6324a68a8df63af1d01edd90c07a4248b5c5fd8bd75c61f57c4f913306`.
Both sides use the same diagnostic-only histogram and harness. The host is an
Apple M4 Max, 16 logical CPUs, 64 GiB RAM, macOS 27.2 / Darwin 27.2.0 arm64;
Node 26.12.1, Playwright 1.56.1, rustup Rust 1.99.0 and pinned Binaryen 132
(`cargo --release`, opt-level 2/LTO, then `wasm-opt -O3 -g`). Measurements were
made locally on 2026-11-07 UTC (2026-11-08 JST), one emulator at a time, without
concurrent builds or regression suites. This is a fast host, not evidence of
adequate speed on every ordinary browser device.

In the four-second Chromium production Worker profile, 3,191 host samples
included 1,889 idle samples. Of the remaining 1,302 samples:

| Host function/category | Self samples | Non-idle share |
| --- | ---: | ---: |
| CPU execute_step (includes inlined decode, dispatch, rollback) | 526 | 40.4% |
| Operand read_mem/write_mem/check_mem | 134 | 10.3% |
| Bus tick | 136 | 10.4% |
| TimingState finish | 112 | 8.6% |
| Exported scheduler loop | 86 | 6.6% |
| PIT Counter tick | 38 | 2.9% |
| FM operator synthesis | 15 | 1.2% |
| Physical Bus read | 9 | 0.7% |

Sampling cannot separate inlined rollback, memory translation, cache accounting
and decode within execute_step; these percentages are not exclusive subsystem
totals or proof that a particular inline operation dominates. No per-instruction
trace or copyrighted guest data was collected. The separate renderer's cost is
measured by its own counters, not attributed to this Worker profile.

The baseline takes about 3.6–3.9 ms of guest execution per 10 ms slice versus
about 1.1–1.2 ms of raster work per presented frame at 25–27 fps. Worker snapshot
preparation is measured independently. Disk counters rise by 130 floppy sector
reads or 260 IDE sector reads across the transition. The short first-response
transition probe includes polling/presentation delay; it does not isolate DOS
decompression time. CPU work remains the largest measured producer cost.
The chosen guest stays near real time in Chromium with no production underruns;
there is no evidence here of sustained PCM underproduction, a guest-clock
regression or a render-Worker-caused slowdown. This does not disprove the
reported slowdown in another game or on a slower host.

The selected build change enables LLVM's standard Wasm SIMD target feature for
state copies in the sampled CPU path. Host-code inspection found 217 vector
loads and 219 vector stores in that CPU function. Rust's IEEE
arithmetic rules remain in force; no fast-math, native guest execution, threads
or new dependency is introduced. The interpreter's Rust source is unchanged.
CPU tables, segment/page checks, every physical byte access, cache/bus
accounting, journaling, device ticking, synthesis, audio watermarks, frame cadence
and Worker topology remain unchanged. The bundle uses Wasm SIMD; both measured
browser engines must compile and execute the actual artifact.

### Rejected experiment

Blanket `inline(always)` on read_mem/write_mem/check_mem increased the Wasm
bundle from 548,889 to 585,829 bytes (+6.7%). Chromium's four-guest-second
capacity times (three runs each) were:

| Medium | Baseline ms | Blanket-inlining ms |
| --- | --- | --- |
| Floppy | 1333.0 / 1364.9 / 1460.3 | 1333.1 / 1331.1 / 1335.6 |
| HDD | 1342.8 / 1338.8 / 1355.1 | 1325.7 / 1544.6 / 1296.9 |

Median gains of about 2.4%/1.3% were small relative to run variation. Production
floppy slice means actually increased (inline 3.638→3.714 ms; Worker
3.598→3.650 ms), although HDD means improved. All Chromium windows still had
zero missing samples and deterministic counters/checksums matched. This did not
justify the size increase, so blanket inlining was removed. Its Firefox run was
stopped after this rejection; no completed Firefox result is claimed for that
candidate. Final measurements below refer to SIMD, not this experiment.

Resolving the segment's first linear address during preflight, then reusing its
base across operand bytes, also failed screening. Chromium four-second times
were 1361.3/1421.9/1335.4 ms on floppy and 1417.7/1350.4/1380.3 ms on HDD:
about +0.3%/-2.7% median capacity relative to baseline. That change was removed.

Reducing the exact clock ratios to 4/5 and 4/33 with constant divisors gave
Chromium times 1368.7/1337.7/1343.4 ms (floppy) and 1368.0/1339.5/1337.0 ms
(HDD), only +1.6%/+0.2% median capacity. Firefox's single capacity controls were
10,340→10,448 ms and 11,403→10,646 ms respectively, an inconsistent result with
no repeatable Chromium gain. Exact guest/PCM/frame counters matched throughout;
the arithmetic change was nevertheless removed. The full-frequency phase
oracle remains a small original regression for future host optimizations.

Bounding Binaryen's single-caller inlining at a size limit of 1,000 instead of its
unbounded default reduced the largest host Wasm function from 127,043 to 68,782
bytes. Chromium capacity times were 1350.0/1354.4/1353.1 ms (floppy) and
1373.1/1348.0/1350.9 ms (HDD), about +0.9%/-0.6% median capacity. Firefox's
single controls were 10,340→11,068 ms and 11,403→10,490 ms, again inconsistent.
All deterministic counters/checksums matched. The build limit was removed;
smaller functions alone did not establish a performance benefit.

## SIMD results and limits

The selected Wasm is 546,963 bytes (baseline 548,889), SHA-256
`01168eb2544cf4c731ae9bd80bd5dd474184a505ed7004900fde422d20e5a1c6`.
The [derived numeric report](runtime-performance-results.json) contains the full
per-window counters, artifact/harness hashes, profiles and repeat observations.
It contains no game bytes, guest text, frames, PCM or disassembly. Chromium is
141.0.7390.37; Firefox is 142.0.1. Chromium uses three repetitions per path;
Firefox uses one per path, with no statistical-repeatability claim.

### Fixed-work capacity

Four guest seconds, identical input/scene steps, host milliseconds:

| Engine / medium | Baseline runs | SIMD runs | Median capacity gain |
| --- | --- | --- | ---: |
| Chromium / floppy | 1333.0 / 1364.9 / 1460.3 | 1286.6 / 1279.3 / 1351.6 | 6.1% |
| Chromium / HDD | 1342.8 / 1338.8 / 1355.1 | 1284.4 / 1296.6 / 1290.1 | 4.1% |
| Firefox / floppy (one run) | 10340 | 10282 | 0.6% |
| Firefox / HDD (one run) | 11403 | 10258 | 11.2% |

A separate SIMD screening series was 1302.9/1285.0/1301.0 ms (floppy) and
1277.6/1284.9/1291.0 ms (HDD). Restoring the original artifact afterward gave
1347.1/1337.1/1329.5 ms and 1346.8/1328.3/1408.3 ms. Against that fresh
baseline, screening median gains were 2.8%/4.8%. These reverse controls support
a **modest active-execution benefit**, not a precise universal speedup.
The single Firefox values cannot establish repeatability; retain the small
floppy result as well as the larger HDD result.

All repeats retain the following exact workload observations across builds:

| Medium | Guest seconds | CPU clocks | Completed instructions | Sector reads | PCM samples | FM key edges / sound IRQ edges | PCM FNV-1a32 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Floppy | 4.000074125 | 263806891 | 17304475 | 130 floppy | 192004 | 112 / 577 | 3111521906 |
| HDD | 4.000061750 | 264004069 | 17316244 | 260 IDE | 192003 | 112 / 578 | 3227959969 |

Both end-frame FNV-1a32 values are 1922511076. These are in-memory numerical
checksums, not captured media or cryptographic proofs. Existing literal CPU,
audio, graphics and disk regressions remain necessary. The slight deadline
overshoot is the unchanged finite-instruction contract. Firmware disk waits
also advance device time, so CPU clocks are not simply guest seconds × 66 MHz.

### Production Chromium

Three-run medians; arrows are baseline → SIMD. These use the actual scheduled
Worker and AudioWorklet, including a scene transition; they are not the manual
capacity test.

| Medium / renderer | Guest seconds/wall second | Slice mean ms | Slice p95/p99 ms | Presented fps | Browser CPU seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Floppy / inline | 1.0002 → 1.0001 | 3.638 → 3.742 | 4.2/5.5 → 4.4/5.3 | 26.05 → 25.19 | 3.92 → 3.94 |
| Floppy / Worker | 1.0002 → 0.9998 | 3.598 → 3.794 | 4.2/5.7 → 4.3/5.4 | 26.50 → 25.06 | 3.83 → 3.99 |
| HDD / inline | 1.0001 → 1.0007 | 3.910 → 3.783 | 4.3/5.6 → 4.3/5.4 | 25.40 → 25.03 | 4.07 → 3.97 |
| HDD / Worker | 0.9997 → 0.9999 | 3.708 → 3.721 | 4.3/5.5 → 4.3/5.4 | 26.34 → 25.23 | 3.91 → 3.93 |

All 24 production Chromium windows had **zero missing samples, overflow,
producer drops and frame coalescing**. PCM queue observations remained
2400–2401 samples. Production and consumption stayed about 48,000 samples/sec,
instructions about 4.24 million/sec, with 232 FM key edges per window and
changing FM/SSG pitches. Exact per-window samples, clocks and IRQ counts are in
the numeric report. Ratios just below 1 here reflect finite-window pacing;
there is no sustained PCM deficit on this host.

First-background-response medians were 75→52 ms / 72→51 ms for floppy
inline/Worker and 50→51 ms / 71→73 ms for HDD. Capture-to-Canvas means stayed
about 5.5–5.8 ms. Neither these polling/frame-quantized response differences nor
the paced CPU/fps results establish a production presentation speedup. In fact,
some slice, fps and CPU observations are worse. Retain this negative result:
**SIMD improves measured fixed-work capacity, not consistently paced playback
cost or presentation**. It does not solve the reported problem for all games.

Browser RSS observations and Wasm memory are recorded separately in the report;
resident shared pages are counted per process. No memory/latency-growth remedy
is used. The standard Wasm SIMD feature is now required by the release bundle;
the evidence applies to these tested browser engines, not older implementations.

### Production Firefox: still below real time

One observation per combination; baseline → SIMD. These **remain failing
real-time/audio results**, not passing continuity evidence.

| Medium / renderer | Guest seconds/wall second | Slice mean ms | Slice p95/p99 ms | Missing output samples | Browser CPU seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Floppy / inline | 0.3721 → 0.3944 | 26.795 → 25.294 | 40.1/41.1 → 37.1/39.1 | 279581 → 273757 | 9.73 → 9.96 |
| Floppy / Worker | 0.3731 → 0.3868 | 26.718 → 25.794 | 39.1/41.1 → 39.1/43.1 | 278557 → 286205 | 9.72 → 10.25 |
| HDD / inline | 0.3778 → 0.3872 | 26.417 → 25.770 | 39.1/41.1 → 39.1/40.1 | 284926 → 284349 | 10.00 → 10.08 |
| HDD / Worker | 0.3804 → 0.3773 | 26.235 → 26.418 | 39.1/41.1 → 40.1/41.1 | 292541 → 280381 | 10.22 → 10.03 |

Windows were 9.30–9.83 seconds before and 9.52–9.66 seconds after, including
the bounded, unsuccessful presentation-response probe. Compare rates, not just
absolute missing-sample totals. PCM production was only about 18–19 kHz versus
the 48 kHz sink requirement, approximately 1.6 million completed instructions/sec
versus the game's approximately 4.24 million/sec real-time requirement. CPU use
was about one core. Queues repeatedly emptied (0–1441 samples), with zero
overflow and producer drops. This is sustained production shortfall, not a
queue-capacity fix or proof of an incorrect guest clock. The render-free
capacity controls independently remain below 1×, and both architectures have
similar slice costs. SIMD does not make this configuration meet the deadline.

The guest completed the additional 130 floppy/260 HDD reads and kept changing
notes/IRQ counts, but **no frames were presented inside any Firefox observation
window**. Thus fps is 0, transition/latency/raster means are unavailable, and no
visible transition completion is claimed. Audio-priority presentation deferral
under sustained starvation remains a limitation. The original 30-second
response-wait attempt timed out while the guest was still advancing; the final
harness bounds that probe and records the slow run instead. Chromium responses
were all within one second, so bounding this negative probe did not change its
baseline scenario. Firefox timer precision also limits sub-millisecond estimates.

Exact fixed-guest-time clocks, instructions, samples, notes, IRQ edges and PCM/
frame checksums still match the baseline and Chromium. Song events progress at
the same rate per **guest** second; the slow wall-clock cadence and missing
samples are not repaired by changing pitch, tempo, interrupts or guest clocks.
This local optimization is not a claim of glitch-free Firefox playback or an
all-game fix. Further work needs a larger interpreter execution gain and
separate investigation of presentation during persistent starvation.

## Validation

This table records the original local validation. The subsequent Actions audio
failure and revised artifact are covered in [Audio acceptance follow-up](#audio-acceptance-follow-up).

Browser checks used the final SIMD artifact and unchanged licensed CI fixtures.
Native checks used Homebrew Rust 1.94.0; release Wasm used rustup Rust 1.99.0
as described above.

| Check | Result |
| --- | --- |
| All 56 Python commands from the README checks block | Passed, including fixture-fetch tests, pinned sources/notices and every fixture/manifest verification |
| `CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true cargo test --locked` | 606 passed across 42 targets, zero failed/ignored |
| `cargo clippy --all-targets -- -D warnings`; `cargo fmt --check` | Passed |
| `npm ci`; `npm run build`; `npm run check:site` | Passed; release Wasm compiles, all 27 approved publication assets verified |
| `node --test tests/audio-worker.test.js tests/audio-worklet.test.js tests/render-worker.test.js` | 20 passed |
| `npx playwright test tests/browser/audio-continuity.spec.js --workers=1` | Both production Worker cases passed at 48/44.1 kHz, `/` and `/pc98-rust/` |
| Existing audio tests with initial URL `?diagnostics=1&renderer=inline` | Both inline cases passed at the same rates/roots; 64 total one-second windows across both paths had zero missing samples |
| `npx playwright test --workers=4 --grep-invert 'ADV98 production playback remains continuous through scene changes'` | 255 passed, 3 existing skips |
| Local-media helper hash/rejection checks, report invariant checks, `git check-ignore`, `git status --short`, `git diff --check` | Passed; no media or new publication assets in the change |

The unchanged audio assertions cover pause/resume, mute/unmute, AudioContext
suspend/resume, visibility recovery, reset during BGM, pitch changes, IRQ/note
cadence, resampling, sample numbering/checksums and bounded queues. The functional
suite retains CPU timing/faults, device/IRQ, graphics, disk, Worker ownership,
render parity/fallback, and packaging/security coverage at both serving roots.

The inline run reused an ignored copy of the existing test, changing only its
initial URL and relative helper imports. Its temporary config initially used
the wrong web-server working directory; startup was corrected before rerunning
successfully. To reproduce that local-only extra run after the normal build:

```sh
python3 - <<'PY'
from pathlib import Path
source = Path('tests/browser/audio-continuity.spec.js').read_text()
source = source.replace("'./helpers/", "'../tests/browser/helpers/")
source = source.replace("await page.goto('./');", "await page.goto('./?diagnostics=1&renderer=inline');")
Path('target/issue144-inline.spec.js').write_text(source)
Path('target/issue144-inline.config.mjs').write_text("""import config from '../playwright.config.js';
import {fileURLToPath} from 'node:url';
const cwd=fileURLToPath(new URL('..',import.meta.url));
export default {...config,testDir:'.',testMatch:'issue144-inline.spec.js',outputDir:'../test-results/issue144-inline',webServer:config.webServer.map(server=>({...server,cwd}))};
""")
PY
npx playwright test --config=target/issue144-inline.config.mjs --workers=1
```

Prepared source archives and CI fixture media remain ignored; benchmark images,
working numeric outputs and host inspection files stayed outside the checkout.
Only self-authored tooling/documentation, an original timing regression and the
derived numeric report are committed. No game bytes, saves, captures, soundtrack
or guest disassembly enter Git or `web/`. No new hosted-CI download/upload is
introduced. Required post-merge verification: None.


## Audio acceptance follow-up

The [current-head Actions run](https://github.com/takahirox/pc98-rust/actions/runs/42391603042)
failed the unchanged continuity assertions: 1,944 missing frames at 48 kHz and
1,687 at 44.1 kHz. Earlier local passes above did not establish CI acceptance.
The retained trace identifies an execution shortfall during bitmap/DOS work:
27 slices advanced approximately 270 ms of device time while spending 309.7 ms
(48 kHz) or 304.3 ms (44.1 kHz) inside guest slices. Their mean slice times were
11.5/11.3 ms for a 10 ms target. The 48 kHz interval captured no video snapshots;
the 44.1 kHz interval captured one, costing 0.1 ms. Neither had producer drops,
overflow or sequence/checksum errors. CPU execution, operand access, timing and
PIC arbitration also appear in the failure-only Worker profiles. A larger audio
queue would only postpone this scene's producer deficit.

This revision reduces that host work in three places:

- Conventional real-mode segments check their complete span directly. Hidden
  descriptor caches, protected mode and paging retain the full validation path,
  outside the frequently inlined fetch sites.
- Standard Wasm SIMD compares both words of the eight registers and extracts
  the exact dependency mask, replacing repeated scalar comparisons. The native
  fallback retains the same value-change semantics, including upper-word-only
  changes and unchanged writes.
- PIC arbitration rotates the request/service masks into priority order and
  compares their first set bits. Exhaustive request/service combinations at all
  eight priority origins retain blocking, tie and special-mask behavior.

The 10/66 MHz CPU models, instruction and bus clocks, device scheduler, IRQ and
synthesis semantics, six audio credits, 60 ms target and 70 ms outstanding bound
are unchanged. The audio tests and their silence/cadence assertions are unchanged.
No test workflow, media or publication allowlist changes are needed.

### Revised fixed-work measurements

Fresh three-repeat controls use the same local ADV98 floppy/HDD media, setup,
four-guest-second scene sequence, Chromium 141.0.7390.37, Apple M4 Max (16 logical
CPUs, 64 GiB), Darwin 27.2.0, Node 26.12.1 and Rust 1.99.0 release-Wasm build as
above. Runs were sequential, with no concurrent builds or regression suites.
The baseline is the reviewed SIMD artifact (`01168eb2544cf4c731ae9bd80bd5dd474184a505ed7004900fde422d20e5a1c6`).
The revised artifact is 537,339 bytes, SHA-256
`3f52117e7e89faa4ae2c53d2d60649c0664302ec12c160751482e35180e8d7fd`.

| Medium | Reviewed artifact ms | Revised artifact ms | Median capacity gain |
| --- | --- | --- | ---: |
| Floppy | 1411.3 / 1315.5 / 1286.3 | 1168.8 / 1174.6 / 1170.2 | 12.4% |
| HDD | 1436.7 / 1284.2 / 1289.2 | 1169.6 / 1175.1 / 1172.5 | 10.0% |

All twelve controls match exactly on device seconds, CPU clocks, completed
instructions, disk reads, PCM samples and hash, frame hash, FM key edges and
sound IRQ edges (the literal per-medium values in the original fixed-work table).
Reproduce with the existing harness's `--capacityOnly=1 --repeat=3`, retaining
separate outputs outside the checkout for each artifact. These are active
execution measurements, not proof of CI performance or adequate Firefox speed.
Screening higher LLVM optimization, CPU layout changes, decoder outlining,
timing-match rewrites and a word-fetch shortcut did not justify retaining them.

A separate exploratory Linux probe multiplied each live Wasm slice's elapsed
cost by 2.5 using a temporary local Worker wrapper (busy-wait until
`start + 2.5 * elapsed`, with the unchanged guest result and audio assertions).
It still fails on both artifacts: the first failing one-second windows contained
2,329/4,820 missing frames before and 2,328/1,742 after (48/44.1 kHz). This probe
is not part of normal acceptance and is not a passing result. The revision
adds measured execution headroom; it cannot guarantee continuity under arbitrary
host slowdown. No larger buffer or relaxed test threshold conceals that limit.

### Revision validation

On 2026-11-07 UTC, all 57 Python acceptance commands from the workflow passed,
including fixture-fetch tests and unchanged license/media manifests. The native
suite passed 608 tests with opt-level 1, debug assertions and overflow checks;
the timing target then passed all 22 tests including the new per-register
upper-word/unchanged-write regression. Native Clippy (all targets, warnings
denied), SIMD Wasm Clippy with the matching rustup compiler, formatting, release
build and the 27-asset publication audit passed. The native toolchain is Homebrew
Rust 1.94.0; the Wasm toolchain is rustup Rust 1.99.0. An initial optional Wasm
Clippy invocation selected Homebrew's driver and could not find its Wasm std;
using the installed rustup toolchain consistently resolved that setup failure.

Ubuntu 24.04 arm64 in the Playwright 1.56.1 container, limited to two CPUs,
passed all 20 Node audio/render tests, then the unchanged production continuity
tests serially at both serving roots. The same tests also passed with only the
initial renderer URL changed to inline, using the existing reproduction above.
All four cases include scene work and every pause/mute/suspend/visibility/reset
assertion, zero drops/overflow/sequence/checksum errors, and bounded queues.

| Renderer / device rate | Observation windows | Produced source samples | Missing output samples | FM key edges | Sound IRQ edges |
| --- | ---: | ---: | ---: | ---: | ---: |
| Worker / 48 kHz | 16 | 798733 | 0 | 472 | 2403 |
| Worker / 44.1 kHz | 16 | 797774 | 0 | 472 | 2401 |
| Inline / 48 kHz | 16 | 799693 | 0 | 472 | 2408 |
| Inline / 44.1 kHz | 16 | 795853 | 0 | 472 | 2395 |

The remaining Linux `npx playwright test --workers=4 --grep-invert
'ADV98 production playback remains continuous through scene changes'` suite
passed **255 cases, 3 existing skips, zero failures (7.4 minutes)**, with four
CPUs/four workers and the same release artifact. The skips avoid duplicating
large-media scenarios under Pages; their root-path cases passed. This retains
both serving paths' CPU faults/timing, PIC/PIT/device/IRQ, graphics/disk,
Worker ownership, render parity/fallback, packaging and security coverage.

The 64 windows observed 1,920–2,401 queued source samples; the existing tests
also enforce fewer than 3,360 outstanding samples and at most six queued
blocks including the currently consumed block. No additional latency is introduced.
These are local production-artifact passes, not a new GitHub Actions result.
The revised head still needs its normal acceptance run after downstream
publication; this revision does not push or merge. The subsequent
[production matrix](#revised-production-measurements-review-follow-up) remeasures
Firefox as well as Chromium for this artifact and records the remaining
throughput/audio limitation. Benchmark media, profiles, traces and working reports
stay outside Git; no new published assets, fixture downloads or hosted uploads
are introduced.

## Revised production measurements (review follow-up)

This matrix validates the CPU/segment/register/PIC revision above against the
reviewed SIMD build, rather than reusing the earlier SIMD-only production data.
Both artifacts use the unchanged production Worker, AudioWorklet, rendering paths
and measurement harness. The fresh comparison uses `ba4b347` (Wasm SHA-256
`01168eb2544cf4c731ae9bd80bd5dd474184a505ed7004900fde422d20e5a1c6`)
and `8ee952a` (Wasm SHA-256
`3f52117e7e89faa4ae2c53d2d60649c0664302ec12c160751482e35180e8d7fd`).
The current artifact was rebuilt locally and its checksum matched before testing.

The host, browser versions, guest configuration and media hashes are the same
as above: Apple M4 Max, 16 logical CPUs, 64 GiB, Darwin 27.2.0 arm64,
Chromium 141.0.7390.37 and Firefox 142.0.1, Ap DX2-66 with unchanged clocks,
and the local ADV98 floppy/HDD music scenes. Node is 26.12.1, Playwright 1.56.1,
Rust 1.99.0, Binaryen 132; the normal release build uses SIMD, opt-level 2/LTO
and `wasm-opt -O3 -g`. The harness SHA-256 is
`bc51367a9d123dafb50aa9cf4a119cb6bb00936c09a998fba03cf90da41bc6eb`;
Worker SHA-256 remains
`918440f74241a2f53f2d47635af94ca3816f77238f0553a61fe12c260e96d2c5`.
No host profiler, compilation, regression suite or second emulator ran during
these measurements. Each engine ran baseline then revision sequentially, with
three repetitions per artifact, medium and renderer; renderer order alternates
by repetition. Fixed artifact order leaves possible time-dependent host drift;
these local medians do not establish a universal effect size.

Each production window still uses 32 polls at 250 ms with Return at the
midpoint, 1.5 seconds of settling beforehand, and the bounded one-second
background-response probe. The separate capacity controls still execute exactly
four guest seconds. No setup, failed presentation or slow engine is silently
counted as a passing playback observation.

### Scheduled production throughput

Three-run medians; every arrow is reviewed SIMD → CPU/PIC revision. The full
per-window CPU clocks, device scheduler cycles, instructions, disk reads and
histogram percentiles are retained in `revisionProduction` in the
[numeric report](runtime-performance-results.json). Timed browser observations
and render-free capacity controls remain separate.

| Engine / medium / renderer | Guest seconds/wall second | Million instructions/s | Slice mean ms | Slice p95/p99 ms |
| --- | --- | --- | --- | --- |
| Chromium / floppy / inline | 1.0004 → 1.0000 | 4.244 → 4.242 | 3.930 → 3.661 | 4.3/5.4 → 4.1/5.2 |
| Chromium / floppy / Worker | 0.9999 → 0.9998 | 4.242 → 4.241 | 3.959 → 3.720 | 4.4/5.3 → 4.2/5.3 |
| Chromium / HDD / inline | 0.9999 → 1.0001 | 4.243 → 4.244 | 3.850 → 3.660 | 4.4/5.6 → 4.1/5.2 |
| Chromium / HDD / Worker | 1.0000 → 1.0000 | 4.243 → 4.244 | 3.956 → 3.719 | 4.3/5.7 → 4.2/5.2 |
| Firefox / floppy / inline | 0.3928 → 0.4454 | 1.703 → 1.923 | 25.406 → 22.390 | 38.1/40.1 → 33.1/35.1 |
| Firefox / floppy / Worker | 0.3928 → 0.4452 | 1.703 → 1.923 | 25.429 → 22.420 | 38.1/40.1 → 33.1/35.1 |
| Firefox / HDD / inline | 0.3910 → 0.4460 | 1.696 → 1.928 | 25.538 → 22.389 | 38.1/39.1 → 34.1/35.1 |
| Firefox / HDD / Worker | 0.3924 → 0.4437 | 1.702 → 1.918 | 25.435 → 22.476 | 38.1/40.1 → 34.1/35.1 |

### Scene response and rendering

Response means the first changed background, not full-screen decode completion.
Unavailable means the bounded probe saw no response; these windows remain in
the report. Capture-to-Canvas latency and raster time require presented frames.

| Engine / medium / renderer | Response ms | Presented fps | Latency mean ms | Raster mean ms |
| --- | --- | --- | --- | --- |
| Chromium / floppy / inline | 49.2 → 51.7 | 25.07 → 25.05 | 6.05 → 5.64 | 1.225 → 1.259 |
| Chromium / floppy / Worker | 50.8 → 51.4 | 25.07 → 25.06 | 6.19 → 5.84 | 1.239 → 1.273 |
| Chromium / HDD / inline | 51.8 → 51.3 | 25.29 → 25.03 | 5.67 → 5.97 | 1.221 → 1.267 |
| Chromium / HDD / Worker | 75.0 → 51.6 | 25.05 → 25.08 | 5.85 → 5.75 | 1.242 → 1.274 |
| Firefox / floppy / inline | unavailable → unavailable | 0.00 → 0.00 | unavailable → unavailable | unavailable → unavailable |
| Firefox / floppy / Worker | unavailable → unavailable | 0.00 → 0.00 | unavailable → unavailable | unavailable → unavailable |
| Firefox / HDD / inline | unavailable → unavailable | 0.00 → 0.00 | unavailable → unavailable | unavailable → unavailable |
| Firefox / HDD / Worker | unavailable → unavailable | 0.00 → 0.00 | unavailable → unavailable | unavailable → unavailable |

### Host CPU and memory

CPU is browser-process-tree CPU seconds per observation, not percent of one
core; average cores divide CPU seconds by each observation's wall seconds.
RSS is the summed end-of-window resident memory (shared pages can be
counted repeatedly). These are medians of samples, not a memory-growth test.
Wasm values below are emulator/render linear-memory MiB, separately from RSS;
the full report retains exact byte counts and each window duration.

| Engine / medium / renderer | Browser CPU seconds | Average CPU cores | Browser RSS MiB | Emulator/render Wasm MiB |
| --- | --- | --- | --- | --- |
| Chromium / floppy / inline | 4.10 → 3.88 | 0.497 → 0.472 | 486.2 → 481.5 | 14.00/0.00 → 14.00/0.00 |
| Chromium / floppy / Worker | 4.10 → 3.94 | 0.500 → 0.479 | 497.1 → 494.6 | 17.75/3.12 → 17.75/3.12 |
| Chromium / HDD / inline | 4.02 → 3.90 | 0.489 → 0.473 | 586.0 → 586.5 | 14.00/0.00 → 14.00/0.00 |
| Chromium / HDD / Worker | 4.12 → 3.94 | 0.500 → 0.479 | 596.0 → 593.3 | 14.25/3.12 → 14.25/3.12 |
| Firefox / floppy / inline | 10.02 → 9.78 | 1.040 → 1.042 | 836.0 → 836.4 | 14.00/0.00 → 14.00/0.00 |
| Firefox / floppy / Worker | 10.05 → 9.75 | 1.039 → 1.041 | 850.9 → 849.2 | 18.31/3.12 → 17.75/3.12 |
| Firefox / HDD / inline | 10.06 → 9.72 | 1.041 → 1.042 | 845.1 → 845.6 | 14.00/0.00 → 14.00/0.00 |
| Firefox / HDD / Worker | 10.04 → 9.77 | 1.040 → 1.041 | 856.5 → 855.2 | 14.25/3.12 → 14.25/3.12 |

### Audio production and missing samples

The sink requires 48,000 samples/s. Rates use each observation's actual wall
duration; missing counts and rates are independent three-run medians.
Queue ranges span all polls in the three repetitions, in source samples.

| Engine / medium / renderer | Produced / consumed samples/s | Missing samples | Missing samples/s | Queue range |
| --- | --- | --- | --- | --- |
| Chromium / floppy / inline | 48017/48017 → 47999/47999 | 0 → 0 | 0 → 0 | 2400–2401 → 2400–2401 |
| Chromium / floppy / Worker | 47996/47996 → 47993/47993 | 0 → 0 | 0 → 0 | 2400–2401 → 2400–2401 |
| Chromium / HDD / inline | 47994/47994 → 48004/48005 | 0 → 0 | 0 → 0 | 2400–2401 → 2400–2401 |
| Chromium / HDD / Worker | 47999/47999 → 48001/48001 | 0 → 0 | 0 → 0 | 2400–2401 → 2400–2401 |
| Firefox / floppy / inline | 18853/18853 → 21377/21377 | 280349 → 248828 | 28942 → 26635 | 0–1441 → 0–1920 |
| Firefox / floppy / Worker | 18856/18856 → 21371/21371 | 283325 → 248829 | 29090 → 26567 | 0–1441 → 0–1921 |
| Firefox / HDD / inline | 18766/18766 → 21409/21409 | 282077 → 248061 | 29178 → 26601 | 0–1440 → 0–1440 |
| Firefox / HDD / Worker | 18834/18884 → 21297/21297 | 282493 → 249597 | 29246 → 26603 | 0–1441 → 0–1441 |

### Matching fixed-work controls

Four guest seconds with identical deterministic scene input, host milliseconds:

| Engine / medium | Reviewed artifact ms | Revised artifact ms | Median capacity gain |
| --- | --- | --- | --- |
| Chromium / floppy | 1270.3 / 1274.5 / 1273.0 | 1180.2 / 1174.5 / 1176.8 | 8.2% |
| Chromium / HDD | 1263.0 / 1269.5 / 1366.6 | 1163.4 / 1163.9 / 1164.7 | 9.1% |
| Firefox / floppy | 10137.0 / 10175.0 / 10154.0 | 9040.0 / 9011.0 / 9022.0 | 12.5% |
| Firefox / HDD | 10157.0 / 10161.0 / 10160.0 | 8984.0 / 9020.0 / 9011.0 | 12.8% |

### Assessment and checks

Chromium's median guest-slice cost decreased by 4.9–6.8% across the four
production combinations. Guest execution remains paced near 1×, with about
4.24 million completed instructions/s and roughly 48,000 produced/consumed
samples/s. All 24 Chromium production windows have zero missing samples;
queues remain 2,400–2,401 source samples. Presented fps stays about 25 and
first-response differences are mixed and polling/frame-quantized. This supports
lower execution cost and additional headroom, not a general presentation gain.
Raster means increased in all four Chromium combinations; memory observations
also do not establish a consistent reduction.
Every Chromium window changes FM and SSG pitches and has 232 FM key edges;
fixed-guest-time phase/checksum comparisons below remain the stronger invariants.

**Firefox still fails real-time playback.** Revised production medians are only
0.4437–0.4460 guest seconds/wall second and about
21.3–21.4 kHz PCM production for a 48 kHz sink.
Across its twelve windows per artifact, missing output samples total
3,377,020 before and 2,981,113 after; compare the
per-wall-second rates in the table as well as these totals. Both artifacts still
empty the audio queue. The current execution gain does not remove the sustained
producer deficit or establish acceptable music continuity.

All 24 Firefox production windows present zero frames. Scene response and
presentation/raster latency remain unavailable on both renderers, despite the
completed transition disk reads. This retains the presentation-deferral failure;
it does not turn an unobserved response into a successful transition. The
render-free Firefox capacity controls also remain below 1×. Worker/inline
similarity and queue starvation still point to host execution shortfall; more
queue latency or a guest-clock change would not establish a throughput fix.

All 48 production windows have zero overflow and producer drops. Frame
coalescing counts, per-window CPU/device clocks, instruction counts, disk reads,
PCM counts, FM/SSG pitches, note/IRQ edges, preparation/raster costs, browser
CPU/RSS and separate Wasm memory sizes remain in the numeric report. Every
production transition completes exactly 130 floppy or 260 IDE sector reads.

All 24 fixed-work controls match the original literal per-medium guest seconds,
CPU clocks, instructions, disk reads, PCM count/hash, frame hash, FM key edges
and sound IRQ edges exactly. This includes the current artifact in both engines;
its speedup does not come from omitted guest work or altered audio phase.
The controls are still not production continuity evidence. The fresh Chromium
capacity gains differ from the earlier screening medians, so retain both series
and their variation rather than claiming a precise universal percentage.

These measurements were made locally on 2026-11-07 UTC (2026-11-08 JST).
Each report records its completion timestamp. Reproduce using the commands in
[Reproduction](#reproduction) for each artifact installed at ignored
`web/core.wasm`, with `--repeat=3 --seconds=8`, omitting `--profile=1` and setting
`--revision` and a fresh external `--output` for each engine/artifact. Build the
two source revisions separately with the same toolchain; retain identical
harness, Worker and media hashes. The committed `revisionProduction` data keeps
all 48 windows and 24 capacity controls, including slow and unavailable results.

For this measurement-only follow-up, `npm ci`, `npm run build` and the 27-asset
publication audit passed. The unchanged harness completed all combinations.
Report checks verified matching before/after browser versions, host, harness,
Worker and media, derived throughput rates, all deterministic control invariants,
and restoration of the current Wasm checksum after testing. Documentation links,
JSON parsing, ignore rules, tracked-file scope and whitespace were checked.
Benchmark media and working outputs remain outside the checkout; only this
self-authored report and its derived numeric JSON are added to the commit.
Existing native/browser regression results above remain their dated observations.
The Firefox limitation and fast-host/single-game scope remain explicit; these
measurements do not claim an all-game fix or close the broader performance gap.
