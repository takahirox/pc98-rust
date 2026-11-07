# Independent browser rasterization (#142)

The emulation Worker exclusively owns the Machine, CPU, bus, RAM/VRAM, IRQs,
disks, FM/PCM synthesizers and virtual clock. The render Worker instantiates the
same release Wasm module but calls only `input_buffer` and `video_render`.
Thread-local initialization is lazy: these exports construct a pixel buffer and
snapshot renderer, never a Machine or devices. Native/inline `Bus::frame` and the
snapshot renderer share `VideoView::frame`; CPU, device and instruction timing
code is unchanged.

## Snapshot and ownership

`src/video.rs` defines version 1 (`V98\x01`) as explicit little-endian bytes,
without Rust layout/padding, pointer, GDC FIFO, or Bus serialization:

| Offset | Bytes | Contents |
| --- | ---: | --- |
| 0 | 128 | Version, profile, display flags/page, guest cycles, both GDC scanout settings |
| 128 | 816 | 16-color and 256-color RGB palettes |
| 944 | 6,016 | All 188 writable Kanji patterns |
| 6,960 | 16,384 | Text words and attributes, including high Kanji bytes |
| 23,344 | 196,608 / 524,288 | Full VM / Ap graphics VRAM, including inactive pages |

Total: **219,952 bytes VM / 547,632 bytes Ap**. Immutable ANK/Shinonome glyphs
are already in the licensed module. Mode flags include 40-column text,
semigraphics, CG dot access, 200-line graphics, analog/PEGC and text enablement.
GDC state includes scroll partitions, pitch, start, character height, cursor and
blink parameters. Snapshot guest cycles drive blink/cursor phase. GRCG and EGC
are ordered guest write engines: their scanout results are captured in VRAM;
their mutable write latches must stay with the CPU. PEGC bank writes likewise
resolve before capture, while scanout page/start/pitch and palettes are included.

Capture runs synchronously between finite guest slices after PCM delivery. The
CPU cannot write during capture. No cross-worker mutable views or shared memory
exist. Snapshot parsing validates version, exact profile-specific length, mode
and page bounds before rasterization. The envelope's guest-clock stamp must match
the bytes. Each output carries machine generation, video epoch, monotonically
increasing sequence and a capture timestamp using `performance.timeOrigin +
performance.now()` (Workers have distinct time origins).

There is one reusable maximum-size transferable snapshot and one 1,024,000-byte
RGBA buffer. The render Worker retains the snapshot until the DOM presents or
discards its RGBA output, then returns the snapshot over its direct emulation
MessagePort. Only that exact sequence returns the credit. While it is away,
requests coalesce; no new snapshots or raster jobs queue. The next capture always
contains the entire newest state. Palette, mode, page, glyph or register changes
therefore need no dirty-region invalidation, even after an arbitrary number of
drops. The wire buffer has fixed maximum capacity, but VM parsing/copying uses
only its declared VM length. Transfer capacity is 1,571,632 bytes, versus
1,024,000 inline. Inline fallback allocates a replacement pixel buffer only when
needed, after terminating a failed renderer; it does not resume or reset the guest.

Capture copies VRAM into a reusable Rust vector and then into the transferable.
The renderer copies that snapshot into its independent Wasm and copies completed
RGBA into its transferable. This adds three snapshot-size memory copies per
frame relative to inline rasterization. Transfer itself does not clone the
buffers. The render instance reserves extra Wasm memory and module/runtime
resources; the tables below report observed Wasm capacity, not process RSS.

## Control and failure behavior

Machine generation changes on reset/profile replacement. Independent video epochs
invalidate in-flight images on pause/resume, reset/profile, mount/eject, HDD import
barriers, export barriers, renderer fallback and context restoration. Epoch
notifications travel directly from the authoritative owner to the DOM; the DOM
also accepts newer stamped state on a frame, so different port arrival orders
cannot strand a paused reset's initial image. Lower epochs/generations/sequences
never present. Control responses retain their existing ordered RPC barriers.
AudioWorklet has an independent PCM port, epoch and six credits; it never waits
for render acknowledgments. Audio delivery precedes optional capture. Both render
paths wait for the PCM reserve before launching work, with a bounded 100 ms
deferral to retain preview refresh on an underspeed host. Cheap snapshot capture
does not make the independent rasterizer free: it still competes for the host's
CPU budget with guest execution, especially during scene changes.

Render startup errors/timeouts, runtime exceptions, invalid snapshot/credit
messages and message deserialization errors terminate the renderer and switch to
the inline path on the same guest. Error reason/startup cost remain in `pc98.stats`.
A lost Canvas context discards frames while returning credits; restoration requests
a fresh full snapshot. Losing the authoritative emulation Worker mutes audio and
stops controls as before. Reload restarts the default two-Worker path. No automatic
restart loop, mandatory OffscreenCanvas, SharedArrayBuffer, COOP/COEP, remote asset
or permission is added. Relative module URLs and the strict asset/CSP/license audit
cover both `/` and `/pc98-rust/` static hosting.

## Reproduce measurements

Prepare the legal fixtures and release build using the README commands. Measurements
are opt-in and have no machine-speed CI threshold:

```sh
node tools/measure_render.mjs --browser=chromium --repeat=3 --seconds=8 --output=target/render-chromium.json
node tools/measure_render.mjs --browser=firefox --repeat=1 --seconds=8 --output=target/render-firefox.json
```

The script serves `web/` locally, launches one real headless browser guest at a
time and alternates inline/worker order. Inline uses the former architecture
(`core.frame()` plus RGBA copy in the emulation Worker, main-thread presentation)
with the same rasterizer, optimized Wasm, guest pacing and instrumentation as the
worker path. It is an architecture control, not a historical binary benchmark.
There is no accelerated stepping inside the observed playback windows.

Workloads: VM STAR TRAIL with guest-clocked beeper audio; Ap 98VIDEOP decoding eight
graphics frames with PCM and returning to DOS; Ap ADV98 `music-scenes.hdm` with
FM/SSG BGM and a bitmap/sprite scene transition. Only silent ADV98 DOS/compiler/title
setup advances via the existing bounded inspection API; live playback uses the
production scheduler. Each observation covers at least eight wall seconds, samples
PCM occupancy every 250 ms, and includes a mid-window ADV98 Enter transition.
Video transition time is reset-to-`VIDEO RETURNED`; ADV98 transition time is
Enter-to-changed background pixels (20 ms polling and browser automation limit its
resolution). Guest time alongside transition wall time distinguishes pacing from
host work. Timer-delay p95/max are 16 ms main-thread interval lateness. Frame
latency is capture-to-Canvas update, not physical display scanout. `coalesced`
counts suppressed snapshot requests; it is not a measured monitor/vsync drop rate.

Producer preparation includes Rust capture/rasterization and its outbound copy;
render-worker time separately includes snapshot import/rasterization/RGBA copy.
Transport timing includes producer preparation before arrival. Diagnostics include
finite guest-slice cost, frames, coalescing/stale outputs, Canvas cost, guest/wall
ratio, queue occupancy and audio underflow/overflow/drop counters. Manual DOM-clock
regressions deliberately have an independent fake clock; their latency fields
are not performance evidence.

## Browser measurements (2026-11-07)

Raw per-run observations: [Chromium](render-worker-chromium.json). Hardware:
**Apple M4 Max, 16 logical CPUs, Darwin 27.2.0 arm64**, headless Chromium
**141.0.7390.37**, pinned Playwright 1.56.1, production optimized release Wasm.
These measurements were rerun after the PCM-reserve correction in PR head
`86499a9`, from checkpoint `c0e798560aa2862e0871b1c36f9db5b746b3a164`.
Its runtime and benchmark sources match that PR head. Both inline and worker
paths use the final scheduler: video waits for PCM reserve, with the same
100 ms preview deadline. The raw JSON records that source revision and SHA-256
hashes of `emulator-worker.js`, the measurement script and release Wasm, so a
JavaScript-only scheduler change can be distinguished from an unchanged core.
The release build used rustup Rust **1.99.0**, pinned Binaryen **132** (`-O3`),
and Node **26.12.1**. Its Wasm hash differs from the earlier measurements; the
comparisons below use this same new artifact for both paths and engines, and do
not measure the scheduler correction against the old binary.

Three pairs per workload, eight-second windows, one guest at a time, separate
from acceptance tests. Values below are medians of the per-run means; arrows
are **inline → render Worker**.

| Workload | Producer preparation ms/frame | Raster ms/frame | Presented fps | Capture-to-Canvas ms | Guest seconds/wall second |
| --- | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 0.924 → 0.007 | 0.911 → 0.937 | 27.72 → 28.09 | 5.293 → 5.365 | 0.9967 → 0.9965 |
| Ap 98VIDEOP | 0.793 → 0.019 | 0.782 → 0.778 | 25.31 → 25.61 | 5.184 → 5.372 | 1.0020 → 1.0031 |
| Ap ADV98 + BGM | 1.035 → 0.021 | 1.020 → 1.020 | 25.19 → 25.60 | 5.290 → 5.506 | 1.0000 → 1.0005 |

| Workload | Guest slice ms | Producer copy ms/frame | Renderer import + RGBA copy ms/frame | Render-hop arrival estimate ms | Canvas put ms/frame | UI timer p95 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 0.400 → 0.400 | 0.016 → 0.003 | 0.019 | unresolved | 0.028 → 0.030 | 1.8 → 1.8 |
| Ap 98VIDEOP | 4.435 → 4.408 | 0.012 → 0.008 | 0.026 | 0.142 | 0.028 → 0.032 | 1.8 → 1.6 |
| Ap ADV98 + BGM | 3.512 → 3.504 | 0.018 → 0.007 | 0.025 | unresolved | 0.030 → 0.031 | 1.8 → 1.6 |

The render-hop estimate includes producer preparation; inline has no extra render
hop. Cross-Worker time origins and privacy-related timer quantization limit these
sub-millisecond estimates: VM and Ap music raw medians were -0.063 and
-0.049 ms, respectively, which are unresolved rather than physical negative
transport costs. Raw observations are retained; tiny arrival/latency differences
should not be interpreted as wins.
End-to-end latency includes all messages and rAF wait. Worst main-thread
timer lateness was **2.9 ms inline / 2.8 ms worker** across these runs. All runs
reported **zero audio underflow, overflow and dropped PCM**, and zero
coalesced/stale frames in these unforced windows. PCM occupancy was 1,920–2,401
source samples (40–50 ms), with 2,400–2,401 during STAR TRAIL/ADV98. Forced
stall/drop/fault behavior is covered by deterministic regressions rather than
timing thresholds.

Video reset-to-return medians were **2967 → 2929 ms**;
ADV98 first-background-response medians were **59 → 50 ms**.
These small differences are within polling/automation/frame-cadence resolution;
they do not establish a full-scene completion speedup. Guest throughput stays at
about 1× because the configured guest clock/audio sink already paces both paths
in real time on this host. The measurable benefit is producer headroom:
approximately **42–131× less rendering work in the CPU/audio Worker**, with
**1.3% more presented STAR TRAIL frames**. Capture-to-Canvas latency regressed
slightly for all three workloads (about 0.07–0.22 ms); UI timer changes are small
and mixed. This is not evidence that every scene or host is faster. The inline
comparison/fallback remains available.

Observed Wasm capacities were **13.44 → 13.69 + 3.13 MiB** for VM and
**14.00 → 17.75 + 3.13 MiB** for Ap (emulator + renderer). Extra transfer capacity
is 547,632 bytes. In addition to module/runtime resources, reusable snapshot
vectors and allocator history increase committed Wasm capacity; this is a real
cost, not process-RSS accounting. Renderer startup medians were
3.4–3.6 ms. The extra snapshot copies and render-hop arrival costs are
included above.

A separate 100-iteration 640×400 ImageData presentation probe measured DOM
`putImageData` at **0.017 ms**, worker OffscreenCanvas put at **0.018 ms**, and
worker put + `transferToImageBitmap().close()` at **0.091 ms** (before any bitmap
message/presentation cost). These measurements favor offloading the existing Rust
rasterizer and keeping the small DOM put. Permanently transferring the visible
Canvas would complicate fallback and context recovery without a demonstrated
benefit here; merely moving put would leave the material producer cost intact.
These refreshed measurements include the audio-priority correction below.
Cheap capture alone does not justify launching render work while PCM reserve is
depleted; both paths defer optional video work until refill or the bounded
preview deadline.

### Firefox comparison and remaining limit

[Firefox raw observations](render-worker-firefox.json) use **Firefox 142.0.1**
on the same hardware and exact Wasm hash, one pair per workload, with no
acceptance tests running alongside. This is a second-engine observation, not a
repeatability/statistical claim. Arrows again mean inline → render Worker.

| Workload | Guest seconds/wall second | Presented fps | Capture-to-Canvas ms | Guest slice ms | Transition ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 1.001 → 1.002 | 20.56 → 27.87 | 13.35 → 14.05 | 2.83 → 2.92 | — |
| Ap 98VIDEOP | 0.291 → 0.346 | 12.68 → 9.26 | 14.07 → 13.99 | 27.99 → 28.59 | 10042 → 8345 |
| Ap ADV98 + BGM | 0.274 → 0.345 | 13.63 → 10.04 | 16.46 → 16.97 | 28.49 → 28.81 | 165 → 189 |

VM presentation throughput improved about **36%**, with no audio underflow.
Ap video guest throughput improved about **19%**, and reset-to-return fell about
**17%**. Ap music throughput improved about **26%**, but its first-background
response regressed **165 → 189 ms**. The latter measures a single response with
polling/automation overhead, not full-scene completion. Both Ap paths remain
**far below real time**: a 10 ms guest slice takes about 28–29 ms before rendering.
Extra video parallelism cannot make that intrinsically ordered producer meet
its audio deadline. No guest clock, sample pitch/tempo or sample numbering was
changed to hide this limit.

| Firefox workload | Underflowed output samples per wall second | Total underflows in respective windows | PCM queue range, inline / worker |
| --- | ---: | ---: | ---: |
| Ap 98VIDEOP | 32093 → 29594 | 321535 → 258975 | 0–1920 / 0–1440 |
| Ap ADV98 + BGM | 35102 → 31346 | 321917 → 280957 | 0–1440 / 0–1440 |

**Firefox Ap playback is not glitch-free on this measured configuration**, in
either architecture, even with the final PCM-reserve deferral. The worker path
improves guest/audio production but regresses Ap presentation fps by about
26–27%. Frame latency increases for VM and Ap music, while Ap video latency is
nearly unchanged. Do not infer universal performance success from the additional
Worker. Latest-frame coalescing under this load discards presentation requests
while bounding memory; video coalescing was 25 → 81 and music 14 → 82 in these
windows. Overflows, dropped producer PCM, and stale presented frames were zero
in all six runs. UI timer p95 was 7 → 3 ms for VM, 8 → 8 ms for Ap video and
9 → 8 ms for Ap music; worst timer lateness was 9 → 11 ms. Wasm/transfer
capacities matched Chromium; render startup was 14 ms. The simpler inline path
remains available when its presentation/copy tradeoff is preferred.

Firefox's coarse timing precision cannot resolve the tiny capture/copy/port
stages reliably: some means round to zero. Raw values are retained as
measurement-resolution limits. Inline producer preparation was 13.73–16.01 ms
per frame versus 0.009–0.025 ms for snapshot preparation (the small values are
not precise speedup ratios). Rasterization remained about 14–17 ms/frame.
The separate presentation probe measured DOM/OffscreenCanvas put at 0.040/0.050 ms
and offscreen put + bitmap creation at 0.210 ms, again providing no evidence to
move the small Canvas operation or add another host-processing Worker.

## Validation

Native snapshot regressions cover VM/Ap scanout parity over page/mode/200-line,
PEGC/palette, GDC partitions/cursor/blink and writable glyph changes, retained
snapshots after subsequent CPU writes, malformed snapshots, and identical guest
memory/device/audio results with capture interleaved. Existing native graphic,
EGC/GRCG, Kanji and timing assertions remain in force.

Node tests exercise real transferable detachment, one-credit ownership, overlap,
reversed sequence, malformed state, clock mismatch and bad credit rejection.
Release-Wasm Chromium integration checks actual render-worker use with zero inline
raster time, full-frame SHA-256 parity after memory growth and repeated VM/Ap
switches, delayed stale-frame discard, newest-state recovery, bounded credits,
startup/runtime/message-fault fallback, and Canvas context restoration at both
serving paths. Existing real ADV98 playback checks retain pitch, guest-sample
cadence, sequence/checksum, overflow, underflow and 44.1/48 kHz coverage.

## Other Worker candidates

FM/PCM synthesis consumes guest register writes and device clocks in their exact
order; moving it to an unsynchronized owner would change observable behavior.
The existing AudioWorklet already owns host sample-rate conversion and the sink;
there is no additional heavy host-only DSP stage to move. Disk import validation
and export run as bounded chunks behind existing media barriers, and there is no
compression/asset-conversion stage in the running application. The measured video
work is independently executable and material. No other candidate currently has
both independent work and measured benefit, so no further Worker is introduced.

### Checks performed for this implementation

- Prepared every fixture command in `.github/workflows/emulator.yml`, including
  pinned legal DOS/music guests and matching sources; fixture-fetch Python tests passed.
- `CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true cargo test --locked`:
  **603 passed**. The strengthened snapshot cases were rerun after adding varied
  glyph/VRAM content to every mode combination: **3 passed**.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and
  `git diff --check`: passed. The moved raster loop also matches the original
  after only the narrow text-view offsets and equivalent page-size substitution.
- `npm test -- tests/browser/audio-continuity.spec.js --workers=1`: build,
  publication audit and real ADV98 playback at both serving paths passed; both
  16-second playback windows reported zero underflow, overflow and dropped PCM.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js tests/render-worker.test.js`:
  **16 passed** after the final protocol regressions were added.
- Full functional Chromium run with four workers: **243 passed, 8 failed,
  3 intentionally skipped**. All eight failures were pre-existing whole-stats
  equality assertions detecting new host render epoch/credit fields during media
  mounts; guest/audio observations were unchanged. Those assertions now explicitly
  compare guest/audio state, excluding host render fields and measured throughput.
  Reran all five affected/control/render suites at both paths: **72 passed**.
  The three skips are the existing project-path duplicate large-HDI cases; their
  local-path counterparts passed. All 253 then-applicable browser cases passed
  across the full run and focused rerun, with the three deliberate skips.

After restricting the PCM-reserve deferral to inline rendering, reran playback,
render and Worker controls serially: **20 passed**, again with zero underflow,
overflow and dropped PCM in both real ADV98 windows. Full-frame parity now
computes SHA-256 inside each Worker/browser context, transferring only the digest
through automation; both serving-path cases passed. Final fault/parity checks
(including a newly added queued-frame/guest-crash regression at both paths):
**8 passed**. The two new crash cases bring applicable unique browser coverage to
255 cases, plus the three intentional duplicate-path skips described above.

Duplicate render-fault notifications are idempotent and cannot mint additional
pixel-buffer credits; the protocol regression passed, and both serving-path
startup/fallback cases passed again (**2 passed**). No deployed-site check or
post-merge verification is required.

### Audio-priority correction after Ubuntu acceptance failure

[GitHub run 42349797543](https://github.com/takahirox/pc98-rust/actions/runs/42349797543)
failed the unchanged serial ADV98 audio gate at both sample rates. Its traces
showed scene-change slices taking 13–15 ms for 10 ms of guest time, versus about
8.7 ms between scenes; snapshot preparation stayed below 0.6 ms. Fast capture
alone therefore did not establish that launching independent rasterization was
safe while the audio reserve was depleted.

The renderer now uses the same PCM-reserve deferral as inline rendering. The
100 ms preview deadline, six audio credits, 60 ms target, guest clocks and all
playback assertions remain unchanged. A deterministic scheduler regression
charges independently scheduled render work against a shared host CPU budget:
the corrected path preserves continuous PCM and bounded preview gaps; restoring
the former exception for the render Worker causes sustained underflows.

This correction is not proof that the required GitHub failure is resolved.
Additional Ubuntu 24.04 ARM64 container experiments on the M4 Max, with the
same release Wasm and Chromium 141.0.7390.37, still failed the unchanged audio
gate in both versions when the entire browser/test process group was restricted
to 0.5 or 0.75 CPU (10 ms quota period). The 0.75 CPU runs reported 540 missed
samples before and 1,432 after in their first failing windows. These are host
budget stress results, not passing acceptance or evidence of a browser speedup.

Validation after the correction:

- Release build and publication audit passed; the Rust/Wasm core is unchanged.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js tests/render-worker.test.js`:
  **17 passed**, including the shared-CPU negative control.
- `npx playwright test tests/browser/audio-continuity.spec.js --workers=1`:
  **2 passed on Mac and 2 passed in the unrestricted Ubuntu ARM64 container**.
  All 64 observed playback windows reported zero underflows; both serving paths,
  sample rates, scene changes and stream-discontinuity assertions passed.
- Functional Chromium suite, excluding that separately run audio test:
  **255 passed, 3 existing duplicate-path cases skipped**.
- Prepared all 56 fixture/media commands from the acceptance workflow. The
  README's optimized native tests passed (**603 tests**), as did formatting,
  Clippy, the 10 fixture-fetch tests and `git diff --check`.

Required Ubuntu x64 GitHub acceptance must pass on the published corrected head;
it cannot be established by a local Mac or ARM64 container pass.

### Refreshed performance evidence after the audio-priority correction

Reran the documented release-Wasm comparisons on the final scheduler, with no
acceptance tests or other benchmark running alongside:

- `npm run build`: release build, media/font checks and publication audit passed.
- `node tools/measure_render.mjs --browser=chromium --repeat=3 --seconds=8 --output=target/render-chromium.json`:
  completed all **18 observations** (three inline/worker pairs per workload).
- `node tools/measure_render.mjs --browser=firefox --repeat=1 --seconds=8 --output=target/render-firefox.json`:
  completed all **6 observations** (one inline/worker pair per workload).
- Copied the complete per-run observations into the linked raw JSON files and
  recorded source/scheduler/script hashes and build tool versions in their context.
  The tables above replace the earlier pre-correction comparisons; Firefox Ap
  underflows, presentation regressions and the music transition regression are
  retained. These measurements do not establish Ubuntu x64 acceptance.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js tests/render-worker.test.js`:
  **17 passed**, including the PCM-reserve/shared-CPU regressions.
- Checked raw-result completeness, finite metrics, recorded source/artifact hashes,
  Markdown links, table calculations and `git diff --check`. Runtime sources are
  unchanged by this evidence refresh.
