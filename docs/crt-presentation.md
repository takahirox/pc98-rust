# Optional RGB CRT presentation (#191)

The Display selector switches immediately without resetting the guest. **LCD /
Original pixels (off)** is the default and preserves the existing square-pixel
8:5 presentation. **Sharp CRT** is the recommended CRT profile; **Soft CRT**
increases horizontal spot softness, and **Strong CRT** increases line structure
conservatively. CRT profiles present the current 640×400 framebuffer at 4:3,
including in full screen (Escape exits). If the browser rejects the Fullscreen
API, the display fills the browser viewport instead; Escape also exits that view. The guest still produces 640×400 pixels.
Unknown future framebuffer dimensions use square pixels until their display
aspect is specified; no global 1.2 stretch is applied to other modes.

| Preset | Maximum scanline modulation | Horizontal neighbor contribution |
| --- | ---: | ---: |
| Sharp CRT | 6% | 3.5% total |
| Soft CRT | 8% | 9% total |
| Strong CRT | 11% | 7% total |

These are artistic tuning values, not measurements of a particular monitor.
The shader samples exact source pixel centers and blends a small amount from the
left/right neighbors only. Japanese glyphs and one-pixel lines keep their vertical
edges. Scanline phase uses source coordinates, with analytic pixel-footprint
attenuation and a fade below three device pixels per original scanline (fully off
below 1.5). Noninteger scaling does not animate the phase. Very small displays
show mostly geometry and softness rather than aliased stripes.

The optional **Faint RGB mask** is off by default. It has a 2.5% ceiling and fades
in only above three device pixels per source pixel horizontally and two
vertically. Its RGB triads also use source coordinates and footprint attenuation.
At ordinary small sizes, checking the box intentionally has no visible effect.
There is no time-dependent modulation, including with reduced motion enabled.
Gamma stays at unity; bloom and curvature are zero. This keeps black black,
restrains highlights, preserves palette differences, and avoids extra render
passes. There are no composite, noise, fringing, flicker or VHS effects.

Only the preset name and mask preference are saved under `pc98-presentation` in
localStorage. Storage failures are harmless. No guest, disk, game, pixel or audio
data is saved by this feature.

## Presentation and fallback

`web/presenter.js` owns a local WebGL2 texture and one fullscreen triangle strip.
Current Chrome/Edge, Firefox and Safari with WebGL2 enabled can use it; browsers
without WebGL2 retain the original 2D display. The automated validation below
identifies the browser actually tested; other browser/GPU combinations may differ.
No runtime dependency, network shader, font, media or license change is involved.

The original `#screen` 2D canvas is updated first and remains underneath the
pointer-transparent GPU canvas. Keyboard focus, pointer capture and relative
mouse coordinates belong to the original screen, using its displayed dimensions.
Changing a preset redraws even a paused guest. Unsupported WebGL2, shader compile
or link failure, and context loss immediately reveal the latest raw canvas and
its original geometry. Restoration rebuilds GPU resources and reuploads the raw
canvas. A GPU failure does not stop frame acknowledgments or restart either
Worker, the guest, or audio. Context loss of the original 2D canvas retains the
existing refresh/credit recovery behavior.

The production upload uses the incoming transferable view with `texSubImage2D`,
allocating texture storage only on source-size changes. No CPU pixel-filter loop
or retained incoming buffer is added. The raw canvas supplies the rare extra copy
needed on settings changes, resize or restoration. ResizeObserver, window resize
and DPR changes resize only the GPU output. Backing resolution is capped at DPR
2 and 2560×1920, also respecting the device texture limit; CSS owns aspect ratio
once. At the cap a very large/high-DPI display may look softer. One 640×400 RGBA
texture is 1,024,000 bytes; the maximum color backing buffer is approximately
19.7 MB, excluding browser/driver copies and the existing raw canvas. There is no
per-frame texture allocation or unbounded query queue.

## Raw versus filtered captures

Core `frame()`/`video_render` pixels and `#screen.getContext('2d').getImageData()`
or `#screen.toBlob()` remain **raw**, including when CRT is selected. Existing
benchmark/golden hashes must continue to use those bytes. A browser screenshot
of the visible screen includes the **filtered** overlay and corrected geometry.
Shader output differences are display differences, not core emulation failures.
Keep game/media captures local unless their rights permit publication; synthetic
pixel probes and the repository's original guest artwork are sufficient for these
checks.

## Diagnostics and reproduction

At `?diagnostics`, `await pc98.stats()` includes cumulative `presenterStats`:
upload count/time, CPU submission time, and asynchronous GPU elapsed time when
`EXT_disjoint_timer_query_webgl2` is available. GPU timing covers upload plus draw;
it is not scanout latency. At most four queries are pending; disjoint samples are
discarded. Unsupported timers report availability false and no samples. CPU
upload/submission durations measure API work, not GPU completion.

Existing `presentationStats` report frames, capture-to-submit latency, stale
frames, replaced pending frames (`coalesced`) and missed 60 Hz slots. Worker
`videoStats.coalesced` counts source requests combined by the bounded credit
protocol; it is not necessarily a dropped visible frame. Compute fps and mean
times from counter deltas over a wall-clock interval. `putMs` now covers raw put
plus optional GPU submission. Guest `cycles`, device sample rate, produced/
consumed samples, underflows, drops and FM key edges remain available independently.

After preparing the README fixtures and installing Chromium:

```sh
npm test -- tests/browser/audio-continuity.spec.js --workers=1
npx playwright test --grep-invert 'ADV98 production playback remains continuous through scene changes'
# Optional constrained-host A/B, same production scheduler and audio assertions:
PC98_PRESENTATION_CPU_RATE=2 npx playwright test tests/browser/audio-continuity.spec.js --workers=1
```

The existing continuity test now runs consecutive off/Sharp/Soft/Strong windows
inside the same live ADV98 music/bitmap workload, preserving its tempo, guest-time,
48/44.1 kHz underflow/drop and scene/input assertions. It logs `CRT presentation
A/B` with fps, guest/wall speed, upload/submit/GPU cost, frame latency, missed
slots, coalescing, underflows, drops and FM edges. The Pages case uses 44.1 kHz and
the interpreter; the local case uses 48 kHz and the default JIT. CPU throttling is
an optional Chromium DevTools host stress setting, not a change to guest clocks.
Compare within each case; do not interpret different scene windows as a precise
GPU benchmark or compare interpreter and JIT as identical workloads.

`presentation.spec.js` covers raw/guest invariance, both profiles, preset
persistence, small and noninteger display sizes, full screen, actual context
loss/restoration, unsupported/compile failure, credit progress and input, upright
shader pixels, black levels, single-pixel edge retention, stable repeated draws,
undersampled mask suppression and unknown-size geometry. Existing CRT/GDC tests
retain their exact glyph/palette/blink/PEGC assertions.

## Local validation, 2026-11-17

Environment: Apple M4 Max, 64 GiB RAM, production Chrome 157 (headless via
Playwright 1.56.1, `use.channel: 'chrome'`), default 1280×720 viewport, DPR 1,
960×720 CRT output. The real release Wasm was built with the normal SIMD and
Binaryen pipeline; the guest workload is the existing ADV98 music-scenes fixture.
Chromium's pinned browser download stalled during installation here, so the
existing installed production Chrome was selected through a temporary config;
the checked-in Playwright config is unchanged. Native fixtures were prepared
from the acceptance workflow; CPULEVEL used its documented mirror via curl after
the local Python certificate chain failed, then passed the existing SHA-256 and
manifest verification. No certificate verification was disabled.

Normal-host sequential A/B windows (about 4.1 s each):

| Device / tier | Preset | fps | Guest/wall | Upload ms | GPU ms | Latency ms | Coalesced requests |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 48 kHz / JIT | off | 59.81 | 0.999 | 0.000 | — | 8.25 | 215 |
| 48 kHz / JIT | sharp | 60.01 | 0.997 | 0.023 | 0.093 | 7.96 | 213 |
| 48 kHz / JIT | soft | 60.04 | 1.001 | 0.024 | 0.098 | 7.73 | 199 |
| 48 kHz / JIT | strong | 59.88 | 1.000 | 0.025 | 0.099 | 7.87 | 190 |
| 44.1 kHz / interpreter | off | 60.11 | 1.002 | 0.000 | — | 7.23 | 166 |
| 44.1 kHz / interpreter | sharp | 59.88 | 1.000 | 0.018 | 0.098 | 7.04 | 165 |
| 44.1 kHz / interpreter | soft | 59.94 | 0.998 | 0.021 | 0.093 | 7.17 | 155 |
| 44.1 kHz / interpreter | strong | 60.01 | 1.000 | 0.018 | 0.103 | 6.97 | 171 |

All eight windows had zero audio underflows, zero dropped samples and zero
missed presentation slots. CPU draw submission averaged 0.005–0.011 ms per CRT
frame. FM key edges were 112–120 per window; the existing full-workload tempo,
IRQ, source/device sample-count and audio-epoch assertions passed. Off/on guest
speed remained approximately real time. The normal output allocates roughly
2.76 MB for its color buffer plus the 1.024 MB source texture, in addition to the
existing raw canvas and driver-owned copies. These are allocation sizes, not a
measurement of total GPU/browser RSS. Actual memory and performance depend on
driver, resolution, display scale and guest workload; these short runs do not
establish low-end GPU or mobile performance.

With Chromium's 2× CPU slowdown enabled through the test's CDP session:

| Device / tier | Preset | fps | Guest/wall | Upload ms | GPU ms | Latency ms | Coalesced requests |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 48 kHz / JIT | off | 60.05 | 0.998 | 0.000 | — | 8.08 | 202 |
| 48 kHz / JIT | sharp | 59.99 | 0.999 | 0.022 | 0.094 | 13.96 | 416 |
| 48 kHz / JIT | soft | 60.03 | 1.000 | 0.032 | 0.097 | 13.76 | 403 |
| 48 kHz / JIT | strong | 59.96 | 1.001 | 0.027 | 0.095 | 13.77 | 408 |
| 44.1 kHz / interpreter | off | 59.87 | 1.001 | 0.000 | — | 7.51 | 181 |
| 44.1 kHz / interpreter | sharp | 60.11 | 0.999 | 0.025 | 0.093 | 7.23 | 170 |
| 44.1 kHz / interpreter | soft | 59.90 | 0.999 | 0.027 | 0.094 | 7.12 | 172 |
| 44.1 kHz / interpreter | strong | 59.88 | 1.001 | 0.023 | 0.094 | 7.30 | 173 |

Again all eight windows had zero underflows, drops and missed slots; both full
continuity cases passed. CPU submission averaged 0.003–0.009 ms per CRT frame.
The constrained 48 kHz CRT windows increased capture-to-submit latency from about
8 ms to 14 ms and combined more capture requests, while retaining 60 fps and
real-time guest/audio progress. This is a host stress sample, not a simulation of
a particular low-end CPU/GPU. Use Original pixels on devices where filtering
competes with gameplay; keep it as the default.

Validation performed in this worktree:

- `npm ci`; `npm run build`: release Wasm, reproducible media/font/license checks
  and exact publication tree passed (31 approved assets).
- `CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true
  CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true cargo test --locked --all-features`:
  649 native tests passed.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js
  tests/render-worker.test.js tests/jit.test.js tests/cpu-budget.test.js`:
  all 56 checks passed.
- Production Chrome through a temporary config importing `playwright.config.js`
  and overriding `use.channel` to `chrome`: the full non-audio suite ran with four
  workers. Four tests initially failed only on Chrome's unsolicited favicon 404,
  after passing their guest assertions; the empty local favicon declaration fixes
  this. All four were rerun successfully with both serving roots. In total,
  275 distinct browser cases passed across the full run and focused reruns,
  including all 14 new presentation cases and both production audio cases. The
  five existing deliberate Pages duplicates (JIT failure and large-HDI scenarios)
  remained skipped. The final focused command ran audio-continuity,
  presentation, crt, fdc-write-multi and opn-ch3 specs with one worker: 24 passed.
- The same audio-continuity spec with `PC98_PRESENTATION_CPU_RATE=2`, one worker:
  both roots/sample rates passed. No other test workload ran during these audio
  measurements. These added A/B observations reuse the existing playback window.
- `git diff --check` passed. The locally viewed screenshot used only the original
  MIT Star Trail artwork; no image or media capture is committed. GPU pixel tests
  check upright output, preserved black and single-pixel lines, stable repeated
  draws, an invisible mask at small sizes and a faint mask at sufficient density.

The tested Wasm is 587,813 bytes, SHA-256
`b1cd154727d229e559aaf6d101d2162887425b2a09d0e48a578f7a83210c53a1`.
The host ran macOS 27.2.1 and Node 26.12.1. Headless production Chrome rejected
native fullscreen permission here, so its viewport-filling fallback and Escape
exit were exercised; both routes share the same aspect/containment rules.
Firefox, Safari, physical displays, mobile and long-duration GPU memory behavior
were not separately measured. No deployment or post-merge check is required for
this static presentation feature.
