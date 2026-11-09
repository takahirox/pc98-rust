# Continued Doukyuusei 2 evaluation — Issue #146

This evaluation starts from merged PR #147 (`df2c804`), using the same clean,
locally installed HDD and deterministic prologue input described in the
[first evaluation](doukyuusei2-performance.md). Proprietary media, installed
images, screenshots, PCM and guest code remain outside version control and
publication. Only timings, counters and fingerprints are retained here.

## Experiments and selected changes

An opt-in host profile of the game still found CPU execution dominant; device
ticks, timing retirement and word-immediate fetches were visible contributors.
Profiled timings are excluded from the performance comparison.

| Candidate | Screening outcome | Decision |
| --- | --- | --- |
| Avoid a sound-tick division before enough cycles accumulate | About 0.5% in one run | Reverted; insufficient evidence |
| Move vertical-sync division behind the armed check | No gain | Reverted |
| Force timing-retirement inlining | About 2% alone; slower when combined with the fetch change | Reverted |
| Validate a two-byte instruction immediate once with paging disabled | Promising screen; only 0.08% median reduction in repeated controls | Reverted; no repeatable benefit |
| Compile the fetch revision at release opt-level 3 | Slower and larger than opt-level 2 | Reverted; build settings unchanged |

One early fetch screening overlapped compilation and is excluded from performance
claims. These single-run screens select experiments; they are not repeatability
evidence. The original and fetch-candidate artifacts are compared in alternating,
isolated controls below, without other browsers, builds or tests competing.

The rejected fetch prototype validated a two-byte code span once, then retained
the same two physical byte reads. Five focused native tests passed, covering
fetch penalties, A20 per byte, EIP wrap, EGC latches, partial segment/length faults,
code permissions and second-page faults with Accessed bits. The prototype and
its tests are removed from the final change because performance did not justify
the additional CPU path. The merged interpreter remains unchanged.

### Fixed-work rejection control

Installed Chrome 157.0.8103.38 on Apple M4 Max, Darwin 27.2.0 arm64, Node 26.12.1;
the same release opt-level 2/LTO/SIMD and Binaryen 132 build as the first evaluation:

| Artifact | Alternating runs, host execution ms | Median ms |
| --- | --- | ---: |
| Merged baseline | 8898.5 / 8135.9 / 8116.1 | 8135.9 |
| Fetch candidate | 8129.4 / 8148.9 / 8017.7 | 8129.4 |

The first baseline is slower; it remains included. The 0.08% median difference
does not establish useful improvement. Every control matches the previous
24.0002795 guest-second result exactly: 118,102,981 instructions, 1,584,018,440
CPU clocks, 131 IDE sector reads, 1,464 FM key events, 934 sound IRQ edges,
1,152,013 PCM samples, PCM FNV-1a32 3284892698 and frame FNV-1a32 1013578406.
The 541,808-byte candidate SHA-256 is
`1e1accb05aaaf3ed941b5c67f1675e7b9a62371599ec58a46dc571dd6c1b8a02`;
the final 539,950-byte baseline remains
`c424157247e0bb99bebecbe34ea3ae66fbd446395866653ee77ac4c9113061cf`.

## Chrome production playback

Two alternating windows per runtime use installed Chrome 157 and the unchanged
baseline core. All executable assets, including the AudioWorklet, are served
from frozen byte buffers. Every window confirms one request for each JavaScript
asset and two for the selected core, using the production render Worker.

| Chrome playback | Original runtime | Revised runtime |
| --- | ---: | ---: |
| Guest seconds / wall second, mean | 1.00008 | 0.99988 |
| Presented frames / second, mean | 25.51 | 25.43 |
| Mean host slice ms, median | 3.4322 | 3.4356 |
| Missing samples, overflow, drops, sequence/checksum errors | 0 | 0 |
| Queue range, source samples | 2400–2401 | 2400–2401 |
| First sampled CG change, wall ms | 3425.8 / 3425.8 | 3425.7 / 3428.0 |

Every window reads 105 IDE sectors; FM key events and sound IRQs continue, with
no renderer failure or emulator error. The first sampled image change is almost
identical, and both versions already sustain real time. This establishes no
Chrome scene-transition or audio-continuity improvement in the measured prologue;
it checks that starvation handling preserves ordinary healthy playback.

The sampler observes 17/18 patch-change events in the original windows and 18/19
in the revised ones, with maximum polling gaps below 29 ms and no event overflow.
These are sampled intermediate changes, not scene counts. The numeric report
records image-change times alongside IDE-read events and guest clocks; it does
not label the last changed patch as a completed scene.

## Reproduced starvation and frozen display

Pinned Firefox 142.0.1 reproduced a concrete failure with this game: the guest
loaded image data but no new frames reached the canvas during a 25.8-second
audio-enabled window. All nine sampled image patches stayed unchanged. The
interpreter needed about 25.6 host milliseconds per 10 guest milliseconds,
producing only about 39% of the required PCM rate.

The previous video gate allowed a low-rate preview only when its estimated
audio reserve reached exactly zero. Delayed consumption reports could leave
that estimate positive while the actual AudioWorklet was already emitting
silence. Short gaps could also start and recover between guest slices, so
polling only after a slice missed the condition entirely.

The AudioWorklet now reports the current missing-sample run and immediately
reports the start and recovery edges. The emulator handles a fresh starvation
notification between guest slices and permits a preview when the existing
deadline is at least 100 ms overdue. The callback checks the audio epoch,
running state and manual mode, and waits until commands and media operations
finish. A recovered stream still needs its ordinary reserve;
historical underruns cannot grant preview permission. The change neither
advances guest time nor changes PCM, clock rates, prebuffer size or credit limits.

The isolated game comparison keeps the CPU artifact **unchanged**:

| Firefox playback | Original runtime | Revised runtime |
| --- | ---: | ---: |
| Wall seconds | 25.802 | 25.966 |
| Guest seconds / wall second | 0.3891 | 0.3774 |
| Presented frames | 0 | 139 |
| Changes in sampled image patches | 0 | 15 |
| Missing device samples | 764,090 | 781,243 |
| Mean host slice ms | 25.600 | 26.314 |
| IDE sector reads | 105 | 105 |

The revised window shows changing image patches within 291 ms of observation
start and later scene/portrait updates. This establishes recovery from a frozen
display while the sink is starved. It does **not** establish the time to complete
a scene transition or fix audio continuity. Rendering competes for CPU: this
single unchanged-core comparison advances guest time about 3.0% more slowly and
misses more samples. Firefox still cannot sustain real time here. An earlier
diagnostic pair likewise observed 0 versus 137 frames; the table uses the final
server-controlled pair rather than mixing harness versions.

Both windows have zero overflow, producer drops, sequence/checksum errors or
emulator faults. The 25 ms visual sampler observed maximum gaps of 67/68 ms with
no event overflow. Independent Node regressions reproduce positive stale reserve,
starvation/recovery between slices, old epochs and short gaps hidden by ordinary
credit reports. Negative controls using the former gate or removing edge handling
fail those display assertions while healthy-stream reserve tests remain intact.

Intermediate fixes that only changed the gate or only exposed current starvation
still presented zero frames; they are rejected diagnostic experiments, not
evidence of the final behavior. All final runs freeze the selected runtime assets
before setup, including the lazily loaded AudioWorklet, and record asset routing
counts to prevent mixed revisions.

The [derived numeric report](doukyuusei2-performance-round2-results.json) retains
all twelve capacity and production controls, artifact/runtime/harness/media
hashes, and sampled image-change/device-read timelines. No image pixels or audio
samples are included. The final server-controlled playback comparisons all use
the same harness; the earlier capacity controls use page routing with no active
AudioWorklet. A Chrome playback attempt that did not intercept the worklet is
explicitly rejected and excluded. Serving frozen executables directly over HTTP
then verified that Chrome and Firefox loaded every selected asset.

## Regression validation

The unchanged legal guest fixtures prepared in the first evaluation were reused.
The final build revalidated media, font and FreeDOS desktop manifests. Game media
is used only by the separate opt-in local controls, never the regression suite.

| Check | Result |
| --- | --- |
| `cargo test --locked` with test opt-level 1 and debug assertions/overflow checks | 610 passed across 42 suites |
| `cargo clippy --all-targets -- -D warnings`; `cargo fmt --check` | Passed |
| `npm test -- tests/browser/audio-continuity.spec.js --workers=1` | Wasm build and 27-asset publication audit passed; 25 Node tests passed; both Chromium audio cases passed (58.8 s), zero missing samples |
| `npx playwright test --grep-invert 'ADV98 production playback remains continuous through scene changes' --workers=4` | 255 passed, 3 existing Pages skips, both serving roots, 4.4 minutes |
| Numeric report checks | All 12 controls, deterministic counters, selected runtime/core hashes and playback invariants verified |
| `node --check tools/measure_doukyuusei2.mjs`; `git diff --check` | Passed |

The three skips are existing additional large-media scenarios sampled only at
the local root; the required media round trips still cover both roots. New Node
cases catch the specific starvation-edge failure and lifecycle guards without
adding another long guest run to normal CI. The final Wasm remains byte-identical
to the baseline. No post-merge verification is required by Issue #146.

## Scope and remaining work

Issue #146 remains open. The Chrome complaint is not fully reproduced on this
fast host; Firefox proves a display-starvation bug and a substantial remaining
interpreter throughput deficit. These controls cover the installed base game's
prologue, not later scenes, Special Disks, save/load or spoken-audio fidelity.
The sound implementation remains approximate. No universal scene-speed or
audio-quality improvement follows from additional CPU headroom.
