# RGB CRT presentation (#191, #195)

The Display selector switches immediately without resetting the guest. **Sharp
CRT** is the default for new visitors, with the RGB mask disabled. **LCD /
Original pixels (off)** remains an explicit choice with square-pixel
8:5 presentation and the safety fallback when CRT is unavailable. **Soft CRT**
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
localStorage. A saved `off`, `sharp`, `soft` or `strong` preset with a boolean mask
choice is restored on reload, including an intentional Original pixels choice.
Absent, malformed or invalid settings use Sharp CRT with the mask off. Storage
failures are harmless: the same default and live controls work without persistence.
Preferences are applied before the first guest frame. No guest, disk, game, pixel
or audio data is saved by this feature.

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
its original geometry. The status explicitly reports original pixels during
fallback; the selector retains the requested preference. Restoration rebuilds
GPU resources and reuploads the raw canvas. A GPU failure does not stop frame
acknowledgments or restart either Worker, the guest, or audio. Context loss of
the original 2D canvas retains the existing refresh/credit recovery behavior.

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

`presentation.spec.js` covers fresh-visit Sharp CRT defaults, all saved presets
and mask choices, invalid/corrupt and blocked storage, raw/guest invariance, both
profiles, small and noninteger display sizes, full screen, actual context
loss/restoration, unsupported/compile/link/allocation/render failure, credit
progress and input, upright shader pixels, black levels, single-pixel edge
retention, stable repeated draws, undersampled mask suppression and unknown-size
geometry. Existing CRT/GDC tests retain their exact glyph/palette/blink/PEGC
assertions.

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
competes with gameplay. These historical measurements preceded the Sharp CRT
default introduced in #195.

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

## Sharp CRT default validation (#195), 2026-11-17

Validated on Apple M4 Max, macOS 27.2.1, Playwright 1.56.1 and its pinned
Chromium 141.0.7390.37 headless shell, using Node 22.23.5 to match CI's Node 22
major version. The normal browser installer stalled; downloading the same pinned
headless archive from Playwright's official CDN with curl completed successfully.
The checked-in Playwright configuration is unchanged. All browser checks used
the actual release Wasm at both `/` and `/pc98-rust/`.

Initial runs used installed production Chrome 157.0.8103.52. The 32 focused
presentation cases and both audio cases passed under Node 26.12.1, but the full
suite stalled between projects. A Node 22 retry reported 291 passed cases and
five existing skips, then stalled during shutdown. Both stalled runners were
stopped; these attempts do not count as successfully completed suite commands.
Final acceptance uses pinned Chromium below.

Checks performed:

- `npm ci` and `npm run build` passed, including reproducible media/font checks,
  release Wasm compilation/optimization and all 31 approved publication assets.
- The acceptance workflow's legal fixtures were prepared and manifest-verified.
  CPULEVEL used its documented mirror via curl after Python certificate-chain
  validation failed; its existing SHA-256 pin was verified. Certificate
  verification remained enabled.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js
  tests/render-worker.test.js tests/jit.test.js tests/cpu-budget.test.js`:
  56 passed under Node 26.12.1.
- `npx --yes --package=node@22 node node_modules/playwright/cli.js test
  tests/browser/audio-continuity.spec.js --workers=1`: both cases passed
  (59.2 s), with no competing test workload during playback.
- `npx --yes --package=node@22 node node_modules/playwright/cli.js test
  --grep-invert 'ADV98 production playback remains continuous through scene changes'
  --workers=4`: 291 passed, five existing Pages duplicates skipped (5.1 min).
  This includes all 32 presentation cases for fresh defaults, saved choices,
  malformed/blocked storage, GPU failures, raw pixels, input and geometry.
- `git diff --check` passed. The temporary Chrome configuration was removed;
  no browser setup or test-runner changes are included in this change.

The existing ADV98 workload measured consecutive approximately 4.1-second
windows for every preset. Representative off/Sharp comparisons on pinned
Chromium:

| Root / execution | Preset | fps | Guest/wall | CPU upload ms | CPU submit ms | PCM underflows / drops |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `/`, 48 kHz / JIT | off | 60.06 | 0.998 | 0 | 0 | 0 / 0 |
| `/`, 48 kHz / JIT | sharp | 59.97 | 0.999 | 0.018 | 0.006 | 0 / 0 |
| `/pc98-rust/`, 44.1 kHz / interpreter | off | 59.92 | 1.000 | 0 | 0 | 0 / 0 |
| `/pc98-rust/`, 44.1 kHz / interpreter | sharp | 59.87 | 1.001 | 0.018 | 0.007 | 0 / 0 |

All eight windows, including Soft and Strong, had zero underflows, drops and
missed presentation slots. Throughput was 59.85–60.06 fps, guest/wall speed was
0.998–1.001, and CRT CPU upload/submission costs were consistent with the earlier
#191 measurements above. GPU timer samples were unavailable in this pinned
headless browser; production Chrome's separate run measured 0.093–0.098 ms per
CRT frame. Guest sample accounting, audio epochs, tempo and scene changes passed
the existing assertions. These observations establish no timing or throughput
regression on this host; they do not establish mobile or low-end GPU performance.
No shader tuning, guest timing, pixel filtering in Workers or framebuffer
ownership changed.

The release Wasm is 590,414 bytes, SHA-256
`436e45cb1ee3dce54b23c25e4c0e5b938f387efc835a9db64fabf7ad579076ea`.
Native Rust tests were not rerun for this browser-default-only change. No
post-merge verification is required.


## Unchanged scanout presentation — Issue #198

The scanout renderer now reports whether its last successful snapshot required
changing retained pixels. This uses the existing exact input/visible-row
comparisons, without a pixel-content hash. The render Worker names the preceding
output sequence when pixels are unchanged and still transfers the complete RGBA
buffer through the existing single-credit protocol.

The presenter reuses raw-canvas/CRT contents only when that named sequence is its
own last accepted frame, from the same renderer, generation and video epoch, at
the same dimensions. A proof for a dropped output therefore cannot hide a changed
frame. Inline rendering retains full presentation. Raw context loss invalidates
the accepted image; display-setting, resize and GPU-restoration redraws remain
independent. The current CRT shader has no time-dependent uniforms.

An exploratory normal-boot game control on the cached Chromium 141 environment
retained approximately 60 fps and zero missing PCM. Its first input window
accepted 1,723 frames with 242 CRT uploads (1,481 reuses); the second accepted
863 with 111 uploads (752 reuses). This is about 86–87% fewer redundant uploads,
not frame skipping: every changed accepted image still updates the raw canvas
and CRT. A single 0.5-CPU control advanced 0.598/0.724 guest seconds per wall
second in its two windows, with actual Worker slowdown 2.64/2.63 times the
unrestricted control. These are selection observations, not a repeated causal
comparison against main. Fresh frozen-baseline/selected repetitions and final
head acceptance are recorded in the implementation PR linked from
[Issue #198](https://github.com/takahirox/pc98-rust/issues/198).

Focused checks cover dropped-frame ancestry, renderer replacement, generation
and epoch barriers, memory growth, raw context restoration, hidden VRAM writes,
and conservative change detection. Native, optimized Wasm, actual-browser and
serial-audio acceptance retain their existing guest/pixel/PCM assertions. Raw
private game media and reports stay outside Git and publication. Required
post-merge verification: None.
