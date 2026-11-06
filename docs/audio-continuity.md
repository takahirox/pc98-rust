# Sustained DOS game audio — Issue #138

The selected workload is **ADV98 v0.1**, the author's playable PC-98 DOS
adventure sample/engine, running under actual FreeDOS(98) with the guest PMD
4.8s driver. The unchanged executable starts the original MIT `guest/play.mml`
score through OPN ports, timer interrupts and software synthesis in the emulator.
No WAV/MP3, host synth, commercial game, NEC ROM or external emulator code is used.

## Fixture selection, rights and reproduction

Run the fixture preparation commands in the [README](../README.md#reproduce-the-checks),
then `npm test -- --workers=4`. For a focused run after the release build, use
`npx playwright test audio-continuity --workers=1`. In the UI, select Ap, insert
`fixtures/dos-apps/music-scenes.hdm` in floppy 1, select floppy boot, reset,
enable sound and press Enter at the title. Enter advances between two backgrounds
and sprite placements while the same song keeps playing. Disks stay in ignored
local directories and are not added to Pages.

The auditable [music manifest](../fixtures/music-manifest.json) records every
on-disk file's SHA-256 and rights, driver/compiler source and binary hashes,
license-notice hashes and redistribution requirements. Its linked
[DOS/application manifest](../fixtures/dos-apps-manifest.json), generated
`fixtures/freedos/manifest.json` and [font audit](../fixtures/fonts/README.md)
complete the provenance chain:

| Component | Exact source and permission evidence | Conditions |
| --- | --- | --- |
| ADV98.EXE | [Author release v0.1](https://github.com/yarufu/pc98/releases/tag/v0.1), matching [engine source 11a9f1c59c6fd8169439d477d5276ee7c0f27566](https://github.com/yarufu/pc98/tree/11a9f1c59c6fd8169439d477d5276ee7c0f27566), root LICENSE, MIT, PokuG stdio.h | Retain APP.LIC. Release artwork, music and effects are excluded. |
| Baseline scenario and palette | [Source 2f482577433f89abd3166e95209f3ffea21b9bbc](https://github.com/yarufu/pc98/tree/2f482577433f89abd3166e95209f3ffea21b9bbc), root MIT LICENSE; `script_サンプル2.txt`, `adv.pal` | Retain APP.LIC. |
| Scene scenario, soundtrack and art | Repository MIT LICENSE; `guest/music-scenes.txt`, `guest/play.mml`, `background()`/`sprite()` in `tools/prepare_dos_apps.py`; NEXT.G98 inverts CLASS.G98's original planes | Retain RUST.LIC. No stock release art or third-party tunes. |
| PMD.COM / MC.EXE | [Published source c620dc95c5e47970e7839cb5f0b7b9ab742d4f46](https://github.com/d2lmirrors/pmd/tree/c620dc95c5e47970e7839cb5f0b7b9ab742d4f46), `readme.txt`: M.Kajihara's 2019-12-25 free-use grant with retained copyright; original ASM headers | Not MIT. Retain PMD.TXT, complete source archive and the documented JWasm adaptations/build recipe. No proprietary OPTASM binary. |
| Build tool | [JWasm a5c4ea03cc0545a15d81a354251b5f534bef7a1b](https://github.com/JWasm/JWasm/tree/a5c4ea03cc0545a15d81a354251b5f534bef7a1b), License.txt, Sybase Open Watcom Public License | Retain compiler archive/notices; compiler is not on the guest disk. |
| DOS boot/kernel/shell | FreeDOS(98) test-20220120-cherrypick distribution, kernel 679eda7eeeac6cc68bad4d3ff9b055e672529731; FreeCOM dbcswip-20220117, source 86a1a89eadbc14d8ff3f52e62dc51e98210de216; pinned URLs/hashes in generated FreeDOS manifest | GPL-2.0-or-later. Retain COPYING and complete matching source archives when distributing. |
| Replacement CG | Shinonome 16 at 053b21e0a11ef5799c1ea3cefc374763add80617; embedded source, AUTHORS and LICENSE in `fixtures/fonts` | Preserve notices including source design acknowledgements; no NEC font dump. |

`music.hdm` and every previous checksum remain unchanged. The new scene image
uses the same binaries, compiler, score, palette, DOS and generated art. Only
SCRIPT.TXT and NEXT.G98 are added/replaced, with hashes recorded. Complete
prepared fixtures may be used locally/in CI and redistributed with the listed
sources/notices. No new public site assets are needed.

[ADVS98 v0.9.0 beta](https://delacannon.itch.io/advs98) was evaluated as an
alternative on 2026-11-06. Its author describes an MIT engine, separate licenses
for DOS/PMD/fonts/art, OPNA rhythm and an 86-versus-26K distinction. No downloadable
sample media were acquired or assumed cleared. A full bundled-media audit was
unnecessary because ADV98 reproduces the delivery defect with FM/SSG and real
scene I/O. This is not a claim that ADVS98's bundled assets are MIT.

## Diagnosis and correction

The previous Worker ran one 10,000-slot slice per relative 10 ms timer. Each
slice produces approximately 480 samples, but browser timer overhead repeatedly
arrived late. There was no deadline correction or sink feedback. The sink started
consuming immediately, with effectively no reserve. The existing music smoke
test manually stepped Wasm and therefore bypassed the defective production pace.

On this macOS arm64 host in production release-Wasm Chromium, the same ADV98
scene fixture under the original Worker tick and original worklet produced about
45,120 samples/sec. Across an eight-second window spanning a bitmap/sprite scene
change, underflows increased by **23,952 output frames (about 6.2%)**. Guest disk
reads increased from 469 to 599, notes kept changing, and both producer drops and
sink overflow stayed zero. This is producer scheduling shortfall, not message
reordering, a device-rate mismatch or an unavailable OPNA instrument. The baseline
reproduction deliberately fails a bounded-silence assertion.

As a second negative control, replacing **only** the new Worker's `tick()` with
the upstream implementation, while retaining the new protocol and prefilling
worklet, makes the final regression fail its actual silence assertion: 2,176
missing frames in a window allowing fewer than 361. The test therefore detects
the pacing defect independently of protocol/diagnostic availability; prefill
alone cannot hide the permanent producer deficit.

The Worker now uses the sink's consumed **source sample** count to target 1,920
outstanding samples (40 ms), counting messages in transit. It executes one finite
slice per task, yields, and stops producing when sufficiently ahead or out of
credits. A late timer can be followed by another short task without accumulating
unbounded wall-clock debt. Muted execution uses absolute 10 ms deadlines with
catch-up bounded to 30 ms and cleared on control barriers. The guest's 8 MHz
clock, instructions, sound synthesis, timer and IRQ order are unchanged.

The sink prefills 1,440 source samples (30 ms) at startup/recovery. Its integer
phase accumulator avoids floating-point drift in sample selection at 44.1 kHz,
retains phase across blocks and preserves source cadence at 96 kHz. It does not
stretch tempo or fabricate source PCM to conceal starvation. Six 1,024-sample
transferable credits still impose the hard 128 ms transport capacity. The normal
watermark can overshoot by one finite slice; it does not grow with playback
duration. Exceptional credit exhaustion and overflow are counted. Hosts unable
to execute the guest in real time can still underflow; this does not guarantee
glitch-free physical audio on an overloaded machine.

## Regression and bounded observations

`audio-continuity.spec.js` accelerates only silent DOS/compiler/title setup.
After Enter, **all playback and transitions use the production Worker timer**,
real Wasm and real AudioWorklet callbacks. Sixteen one-second observations span
sustained BGM and a bitmap/sprite transition. At `/` it uses 48 kHz; the
`/pc98-rust/` run requests a real 44.1 kHz AudioContext, covering rate conversion
without another guest/profile matrix.

FM/SSG registers are also sampled three times inside each observation window.
The initial one-second-only observation aliased the short melody and falsely
reported a stuck FM pitch. A 137 ms probe confirmed distinct C/E/G/octave-C
register values, and the regression now samples within the window to avoid that
alias. No pitch or synthesis change was made to satisfy that assertion.

Assertions compare source PCM counts to deterministic guest cycles, checksum
every fully consumed block independently, check numbered source ranges, bound
queue/in-flight samples, detect persistent silence and require sensible PMD note
counts, changing FM/SSG pitches, sound IRQ12 edges and actual DOS graphics reads.
A DOM event is handled during the transition. The test permits isolated host
noise below 0.5% plus one quantum per window, and no dropout of 100 ms or longer.
The baseline exceeds this by a large margin.

The final full-suite observations, including the scene change, were:

| Device rate | Observation windows | Produced source samples | Missing output frames | FM key edges | Sound IRQ12 edges |
| --- | --- | --- | --- | --- | --- |
| 48 kHz | 16 | 783,504 | 0 | 456 | 2,358 |
| 44.1 kHz | 16 | 784,944 | 0 | 456 | 2,363 |

Both runs had zero producer drops, sink overflow, sequence errors and checksum
errors. Production matched guest cycles to within sample-boundary rounding;
queued source samples at the snapshots ranged from 960 to 1,446. Output cadence
at 44.1 kHz matched the source cadence through the rational phase accumulator.

The same guest also pauses/resumes, mutes/unmutes, suspends/resumes AudioContext,
hides/shows the document with explicit resume, and resets during music. Epoch
barriers recover all six credits; reset must stop the old song. Existing Worker
tests retain profile/VM, input, presentation, media and fault coverage. Node
worklet tests additionally check exact rational source selection at 44.1/48/96 kHz
for three seconds each, starvation/rebuffering, stale epochs, overflow order,
credit recycling and deliberately corrupted PCM detection.

Diagnostics retain counters, not PCM recordings or event logs: produced/sent
samples, drops, consumed source samples, rendered device samples, silence frames,
maximum starvation run, occupancy, epochs, sequence/checksum failures, guest
cycles, FM writes/key edges/pitch, SSG period and sound IRQ12 edges. Checksum work
runs only with the existing diagnostics option. A starved worklet reports at
least every quarter second even without a recycled block. Source cursors reset
per epoch; energy and error counters are cumulative.

## Scope of sound fidelity

The score uses ordinary OPN FM algorithm 7 and SSG tones. It needs no OPNA rhythm
ROM, ADPCM, LFO or exact hardware envelopes, so these cannot explain the measured
missing output frames. Those features remain unsupported/approximate as
documented in [Ap devices](ap-devices.md). No missing rhythm samples were replaced
with invented tones. This validates continuity of this real musical DOS workload,
not universal PC-98 music fidelity or the maintainer's private game. No manual
listening or post-merge action is a gate.

## Validation

On 2026-11-06, macOS arm64, release Wasm, Playwright 1.56.1 Chromium:

- All 55 fixture preparation/check commands from the acceptance workflow passed;
  pinned downloads were reused. Both music images reproduce their audited hashes,
  and every original music-manifest field/checksum remains unchanged.
- `python3 -m unittest discover -s tools -p test_fixture_fetch.py`: 10 passed.
- `cargo test --locked`: 568 passed, including PMD, FM/SSG/timers/IRQ, OPNA
  channel-3, 86 PCM, PIC and VM beeper coverage.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `git diff --check`: passed.
- `npm ci`, `npx playwright install chromium`, `npm run build`: passed. The
  production release Wasm and publication audit retain 26 approved assets.
- `node --test tests/audio-worklet.test.js`: 8 passed.
- The initial full browser run passed 243 cases and reached the pitch-observation
  alias in both new cases; all continuity assertions had passed. After correcting
  the observation, both focused cases passed (48.4 s), including all control
  barriers, with zero underflows/drops/overflows/order/checksum errors.

The final `npx playwright test --workers=4` run at `/` and `/pc98-rust/` passed:
**245 passed, 3 skipped, 0 failed (4.2 minutes)**. Both continuity/control cases
passed with their final device-rate and 100 ms dropout assertions. The three
existing Pages skips avoid duplicating the 1 GiB capacity, large DOS and
occupied-media resource scenarios; their local-root cases passed. Build output,
disk images, recordings and local probe logs are not committed or published.

## Linux continuity follow-up

The [first Actions acceptance run](https://github.com/takahirox/pc98-rust/actions/runs/42168491863)
failed both continuity cases. Its retained counter traces establish actual
producer starvation, rather than an output-clock measurement error: in the
48 kHz failure window, production and playback each advanced only 43,200 source
frames, while the sink inserted 10,240 silent device frames. Playback plus
silence accounted for approximately the elapsed 1.12 seconds. At 44.1 kHz,
the failing window contained 1,212 silent frames. Neither run dropped producer
samples, overflowed the sink, or reported sequence/checksum errors.

The first sink-paced implementation still used a nested `setTimeout(...,1)`
between catch-up slices. Chromium clamps nested timers to at least 4 ms. A host
needing more than 6 ms to execute a 10 ms guest slice therefore cannot keep up,
even though guest execution itself is faster than real time. A watermark and
startup reserve cannot correct this permanent deficit. Focused Linux playback
on a faster host passed even before this follow-up, so an isolated passing run
alone did not explain the Actions failure.

Catch-up now posts a MessageChannel task, retaining one finite 10,000-slot slice
per task and yielding to input/control/credit messages between slices. When
sufficiently ahead, the Worker still waits on a 5 ms timer; it never spins a
full queue. Tokens invalidate pending tasks across pause/manual/media barriers.
The 40 ms target, six credits, guest clock, PCM synthesis and every existing
browser cadence/dropout threshold remain unchanged. Failure messages now include
bounded source, rendered, missing and queued counters.

`tests/audio-worker.test.js` runs the actual scheduler against a deterministic
7 ms slice cost and a 48 kHz sink, with a 4 ms timer floor. The corrected scheduler
has no starvation over five seconds and stays below 2,400 outstanding samples.
Its negative control restores only the previous timer yield and must exhibit
both a production deficit and sustained silence with the same guest/sink model.
It also checks cancellation of stale tasks and that a full sink waits. These
cheap scheduler tests complement the actual release-Wasm/browser guest tests;
they do not substitute for guest playback. A local-only Linux browser probe
that imposed a minimum 7 ms slice cost also reproduced the timer defect in the
real ADV98 scenario: restoring the previous timer yield produced 14,496 missing
frames in its first observation window. Task yielding maintained 49,440–50,880
rendered source frames per window in that probe, but its fourth window inserted
430 silent frames, exceeding the unchanged 382.4-frame allowance under the
artificial CPU load. This optional stress run is not a passing acceptance result;
it illustrates that task yielding cannot guarantee physical host scheduling.
The mandatory suite below uses the unmodified production Worker. No probe media
or recordings are saved as repository/publication artifacts.

Revision checks on 2026-11-06:

- All 56 Python fixture/build commands from the acceptance workflow passed,
  including unchanged music-image checksums. Fixture-fetch unit tests: 10 passed.
- macOS arm64: `cargo test --locked` (568 passed), Clippy with warnings denied,
  formatting, release Wasm build and the 26-asset publication audit passed.
- Ubuntu 24.04 arm64, Docker limited to four CPUs, Node 22.20.0 and Playwright
  1.56.1 Chromium: all 10 Node audio tests passed. Both actual production-Wasm
  continuity/control cases passed with two browser workers and the original
  thresholds. Each played through 16 observation windows and a scene change:

| Device rate | Produced source frames | Missing output frames | FM key edges | Sound IRQ12 edges |
| --- | --- | --- | --- | --- |
| 48 kHz | 795,984 | 0 | 464 | 2,395 |
| 44.1 kHz | 793,584 | 0 | 464 | 2,388 |

Both cases had zero drops, overflow, sequence errors and checksum errors. Queue
snapshots stayed between 960 and 1,446 source frames. Every stream-control and
reset assertion also passed. These Linux container results do not claim that
GitHub Actions has tested the new commit: that requires downstream publication;
this revision node does not push.

The complete Linux `npx playwright test --workers=2` run at `/` and
`/pc98-rust/` passed: **245 passed, 3 existing skips, 0 failed (13.8 minutes)**.
