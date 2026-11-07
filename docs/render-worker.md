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
for render acknowledgments. Audio delivery precedes optional capture. The expensive inline rasterizer still
waits for the PCM reserve; the measured cheap snapshot path does not inherit
that presentation deferral.

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
The raw JSON records the Wasm SHA-256. Three pairs per workload, eight-second
windows, one guest at a time, separate from acceptance tests. Values below are
medians of the per-run means; arrows are **inline → render Worker**.

| Workload | Producer preparation ms/frame | Raster ms/frame | Presented fps | Capture-to-Canvas ms | Guest seconds/wall second |
| --- | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 2.134 → 0.029 | 2.098 → 2.294 | 26.51 → 27.62 | 6.923 → 6.935 | 0.9962 → 0.9964 |
| Ap 98VIDEOP | 0.867 → 0.025 | 0.854 → 0.846 | 25.43 → 25.60 | 5.439 → 5.330 | 1.0019 → 1.0021 |
| Ap ADV98 + BGM | 1.205 → 0.023 | 1.191 → 1.155 | 25.32 → 25.80 | 5.683 → 5.600 | 1.0001 → 1.0003 |

| Workload | Guest slice ms | Producer copy ms/frame | Renderer import + RGBA copy ms/frame | Render-hop arrival estimate ms | Canvas put ms/frame | UI timer p95 ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 0.787 → 0.851 | 0.036 → 0.013 | 0.052 | 0.090 | 0.058 → 0.063 | 1.8 → 2.0 |
| Ap 98VIDEOP | 4.757 → 4.688 | 0.013 → 0.010 | 0.020 | unresolved | 0.025 → 0.031 | 1.7 → 2.0 |
| Ap ADV98 + BGM | 4.059 → 3.983 | 0.014 → 0.010 | 0.023 | 0.051 | 0.031 → 0.027 | 2.0 → 1.9 |

The render-hop estimate includes producer preparation; inline has no extra render
hop. Cross-Worker time origins and privacy-related timer quantization limit these
sub-millisecond estimates: Ap video's raw median was -0.053 ms, which is
unresolved rather than a physical negative transport cost. Raw observations are
retained; tiny arrival/latency differences should not be interpreted as wins.
End-to-end latency includes all messages and rAF wait. Worst main-thread
timer lateness was **5.6 ms inline / 2.9 ms worker** across these runs. All runs
reported **zero audio underflow, overflow and dropped PCM**, and zero
coalesced/stale frames in these unforced windows. PCM occupancy was 1,920–2,401
source samples (40–50 ms), with 2,400–2,401 during STAR TRAIL/ADV98. Forced
stall/drop/fault behavior is covered by deterministic regressions rather than
timing thresholds.

Video reset-to-return medians were **2962 → 2937 ms**;
ADV98 first-background-response medians were **51 → 50 ms**.
These small differences are within polling/automation/frame-cadence resolution;
they do not establish a full-scene completion speedup. Guest throughput stays at
about 1× because the configured guest clock/audio sink already paces both paths
in real time on this host. The measurable benefit is producer headroom:
approximately **34–74× less rendering work in the CPU/audio Worker**, and about
**4.2% more presented STAR TRAIL frames**. Latency/UI timer changes are small and
mixed, including a slight VM latency increase. This is not evidence that every
scene or host is faster. The inline comparison/fallback remains available.

Observed Wasm capacities were **13.44 → 13.69 + 3.13 MiB** for VM and
**14.00 → 17.75 + 3.13 MiB** for Ap (emulator + renderer). Extra transfer capacity
is 547,632 bytes. In addition to module/runtime resources, reusable snapshot
vectors and allocator history increase committed Wasm capacity; this is a real
cost, not process-RSS accounting. Renderer startup medians were
4.3–4.6 ms. The extra snapshot copies and render-hop arrival costs are
included above.

A separate 100-iteration 640×400 ImageData presentation probe measured DOM
`putImageData` at **0.033 ms**, worker OffscreenCanvas put at **0.031 ms**, and
worker put + `transferToImageBitmap().close()` at **0.133 ms** (before any bitmap
message/presentation cost). These measurements favor offloading the existing Rust
rasterizer and keeping the small DOM put. Permanently transferring the visible
Canvas would complicate fallback and context recovery without a demonstrated
benefit here; merely moving put would leave the material producer cost intact.
Measured cheap snapshot capture also justifies retaining the PCM-reserve
presentation deferral only for the expensive inline rasterizer. Both paths still
deliver PCM before optional video work.

### Firefox comparison and remaining limit

[Firefox raw observations](render-worker-firefox.json) use **Firefox 142.0.1**
on the same hardware and exact Wasm hash, one pair per workload, with no
acceptance tests running alongside. This is a second-engine observation, not a
repeatability/statistical claim. Arrows again mean inline → render Worker.

| Workload | Guest seconds/wall second | Presented fps | Capture-to-Canvas ms | Guest slice ms | Transition ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| VM STAR TRAIL | 1.001 → 1.000 | 20.70 → 27.77 | 13.09 → 13.36 | 2.68 → 2.57 | — |
| Ap 98VIDEOP | 0.316 → 0.371 | 14.18 → 10.27 | 12.57 → 12.58 | 25.80 → 26.65 | 9258 → 7675 |
| Ap ADV98 + BGM | 0.287 → 0.366 | 14.28 → 10.69 | 16.28 → 16.32 | 27.15 → 27.23 | 160 → 142 |

VM presentation throughput improved about **34%**, with no audio underflow.
Ap video guest throughput improved about **17%**, and reset-to-return fell about
**17%**. Ap music throughput improved about **28%**. However, both Ap paths
remain **far below real time**: a 10 ms guest slice takes about 26–27 ms before
rendering. Extra video parallelism cannot make that intrinsically ordered
producer meet its audio deadline. No guest clock, sample pitch/tempo or sample
numbering was changed to hide this limit.

| Firefox workload | Underflowed output samples per wall second | Total underflows in respective windows | PCM queue range, inline / worker |
| --- | ---: | ---: | ---: |
| Ap 98VIDEOP | 30936 → 28266 | 285759 → 242239 | 0–1440 / 0–1920 |
| Ap ADV98 + BGM | 34161 → 30587 | 301438 → 263325 | 0–1440 / 0–1440 |

**Firefox Ap playback is not glitch-free on this measured configuration**, in
either architecture. The worker path improves guest/audio production but also
regresses Ap presentation fps and slightly increases frame latency. Do not infer
universal performance success from the additional Worker. Latest-frame
coalescing under this load discards presentation requests while bounding memory;
video coalescing was 18 → 91 and music 14 → 80 in these windows. Overflows,
dropped producer PCM, and stale presented frames were zero in all six runs.
UI timer p95 was 7 → 3 ms for VM and 8 → 8 ms for Ap. Wasm/transfer capacities
matched Chromium; render startup was 13–15 ms. The simpler inline path remains
available when its presentation/copy tradeoff is preferred.

Firefox's coarse timing precision cannot resolve the tiny capture/copy/port
stages reliably: some means round to zero and one render-hop estimate is slightly
negative. Those raw values are retained as measurement-resolution limits, not
physical negative transport costs. Rasterization remained about 12–16 ms/frame.
The separate presentation probe measured DOM/OffscreenCanvas put at 0.050 ms
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
