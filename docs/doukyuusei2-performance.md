# Local Doukyuusei 2 performance — Issue #146

The user supplied PC-98 **Doukyuusei 2** installation floppies and reported slow
image transitions and unstable audio in Chrome. The local investigation installs
and runs that game, rather than treating the existing ADV98 fixture as evidence
about a different title. Game media, installed HDDs, screenshots, guest code and
audio remain local; none are publication or CI assets.

## Installation and reproduction

Build the application with `npm ci && npm run build` and serve `web/` on localhost.
Use the default Ap profile and its FreeDOS HDD, which loads HimemX and `DOS=HIGH`.
Insert the user's Disk A into Floppy 1, then run `B:` and `INSTALL A:` in DOS.
Press a key to start; replace Floppy 1 with Disk B through K when requested and
press a key after each insertion. The installer creates `A:\NANPA2`, changes into
it and reports completion. No Special Disk is needed for this base-game run.

Export IDE HDD to a local path **outside the checkout**, for example
`/tmp/pc98-146/installed.hdi`. Keep this clean installed copy for both revisions;
do not overwrite it with a later session's writes. Run `NANPA2` in the installed
directory, select the color display, and accept the default protagonist name by
clicking END. Mouse input is required to advance the prologue; Return alone does
not advance its dialogue. The local run reached the classroom background,
teacher portrait, dialogue, and Look/Talk controls without an emulator error.

The opt-in harness accepts an already installed local HDD. It does not download
or install the game, publish its files, or capture images/audio. Reports contain
host timings, device counters and in-memory checksums only. Both media and report
destinations must be outside the checkout. It launches an owned browser and
localhost server on port 8077; run one measurement at a time with no compilation,
regression suite or other active emulator competing for the host.

```sh
node tools/measure_doukyuusei2.mjs --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-146/opening.json --repeat=3
node tools/measure_doukyuusei2.mjs --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-146/prologue.json --repeat=3 --gameplay=1
node tools/measure_doukyuusei2.mjs --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-146/playback.json --repeat=3 --gameplay=1 --playback=1
```

Use `--wasm=/absolute/path/to/baseline.wasm` to compare a saved baseline artifact
without replacing the current build. Routing supplies the same selected artifact
to both Workers. Fresh pages and media imports reset each repeat. Add
`--channel=chrome` to test the installed Google Chrome with a fresh isolated
profile; omission selects the repository's pinned Playwright Chromium.
Use `--browser=firefox` for pinned Playwright Firefox (without `--channel`).
The selected Wasm and all four JavaScript runtime assets are frozen before
setup and routed to the page, Workers and AudioWorklet. The report records their
hashes and successful interception counts. Use `--runtime=/absolute/directory`
to select saved `app.js`, `emulator-worker.js`, `audio-worklet.js` and
`render-worker.js` alongside a baseline Wasm.

Capacity mode measures eight guest seconds of the opening, or 24 guest seconds
of the prologue with mouse clicks every 480 guest milliseconds. It drains the
48 kHz PCM every 10 guest milliseconds and hashes its bit patterns; the final
frame is hashed in memory. Eight-second chunks keep each diagnostic inspection
bounded. Sum host execution time across chunks, excluding presentation and
automation; this measures interpreter capacity, not visible transition latency.

Playback mode uses the ordinary production Worker, renderer and AudioWorklet
after deterministic paused setup. It enables audio, resumes, settles for 1.5 wall
seconds, then observes eight wall seconds of the opening or 48 prologue clicks
separated by 80 ms down and 400 ms up. It records actual elapsed wall time,
guest progress, disk/music events, PCM production/consumption, underruns, drops,
queue bounds and presented frames. These windows are time paced, so their exact
instruction/frame counts need not match the deterministic capacity controls.
Add `--visuals=1` to playback to sample nine small CG-area patch fingerprints
every 25 ms, recording changes alongside guest clocks and IDE sector reads.
Reports include observed sampling gaps; fingerprints establish visible changes,
not completed scene loads or physical input-to-display latency. Pixels remain
in the browser. `--profile=1` captures an aggregate Chromium Worker host profile;
its timings must be excluded from unprofiled comparisons.

## Diagnosis and change

A host CPU sample profile of the installed desktop before game launch found
the interpreter dominant, including repeated `check_segment_access` calls
during real-mode execution after HimemX. The
driver's protected-mode round trip leaves hidden segment caches populated.
The old inline shortcut accepted only absent caches, so ordinary DOS/game
instruction fetches and operands repeatedly entered the full descriptor path.

The revised real-mode, paging-disabled path checks the complete operand against
the actual cached limit for expand-up segments. It preserves nonstandard limits
and bases, overflow faults, and SS's distinct fault vector. Expand-down segments,
protected execution and paging retain their existing full validation. CPU clocks,
device scheduling, disk waits, guest music events and sound synthesis are not
changed. The new native regression checks expand-down lower/upper bounds, 16/32-bit
upper bounds, conforming-code interpretation and overflow across all six segments.
Existing tests also cover retained large/small real-mode limits.

## Measurements

The comparison uses main `9b2e0988ede06f7bd5541d4389014ecac508eed7`, Rustup
Rust 1.99.0, release opt-level 2/LTO/SIMD plus pinned Binaryen 132 `-O3 -g`,
Chromium 141.0.7390.37, Node 26.12.1, Apple M4 Max, Darwin 27.2.0 arm64.
The baseline Wasm is 537,339 bytes, SHA-256
`3f52117e7e89faa4ae2c53d2d60649c0664302ec12c160751482e35180e8d7fd`.
The revision is 539,950 bytes (+0.49%), SHA-256
`c424157247e0bb99bebecbe34ea3ae66fbd446395866653ee77ac4c9113061cf`.

Exploratory controls are excluded from the final performance comparison because
an earlier interactive emulator was not reliably paused. The final fixed-work
controls run after that browser has been closed. The derived numeric report
retains artifact, harness and media hashes without storing proprietary content.

### Fixed-work prologue

Three isolated runs per artifact, 24 guest seconds with identical input:

| Artifact | Host execution ms | Median ms |
| --- | --- | ---: |
| Baseline | 9479.3 / 9451.1 / 9633.1 | 9479.3 |
| Revised | 8196.9 / 8200.9 / 8167.0 | 8196.9 |

The median host-time reduction is **13.5%**, equivalent to **15.6% more
fixed-work capacity**. This is CPU/device execution headroom; it is not a
measurement of visible transition completion time.

All six controls produced exactly 118,102,981 completed instructions,
1,584,018,440 CPU clocks, 131 IDE sector reads, 1,464 FM key events, 934 sound
IRQ edges and 1,152,013 PCM samples over 24.0002795 guest seconds. PCM FNV-1a32
was 3284892698 and final-frame FNV-1a32 was 1013578406 in every run. The
[derived numeric report](doukyuusei2-performance-results.json) includes complete
per-run counters and hashes. These checksums establish unchanged results for
this control, not full-game compatibility or physical sound fidelity.

### Production playback

Two isolated windows per artifact use the normal scheduled Worker and
AudioWorklet. The first exploratory baseline window is excluded because it
overlapped closure of the interactive browser; both subsequent baseline windows
run after that browser has closed. This exclusion is recorded in the report.
| Measure | Baseline | Revised |
| --- | ---: | ---: |
| Guest seconds / wall second | 0.99981 | 0.99981 |
| Median mean host slice ms | 4.0335 | 3.9709 |
| Presented frames / second | 26.25 | 25.93 |
| Missing samples, overflow, producer drops | 0 | 0 |
| Queue range, source samples | 2400–2401 | 2400–2401 |
| Produced/consumed source samples per second | 47,990.9 | 47,990.8 |

Every window reads 105 IDE sectors, emits 1,548 FM key events and 909 sound IRQ
edges, with no sequence/checksum errors or emulator stop. Each lasts about
23.33 wall seconds; finite-window guest progress accounts for the small difference
from 48,000 source samples per wall second. Exact observations are in the report.

The paced slice reduction is only about 1.6% and presented fps is slightly lower.
**No visible-transition or audio-continuity improvement is established on this
host:** both artifacts already sustain the configured guest clock without sink
underruns in these windows. The larger fixed-work gain is additional interpreter
capacity; it must not be presented as a 13.5% reduction in visible transition time.

### Installed Google Chrome

The system browser is Google Chrome 157.0.8103.38, distinct from pinned Chromium
141. Additional isolated controls use `--channel=chrome`, with the same media,
artifacts, counters and input schedule. An initial baseline/revision pair is
followed by two observations per artifact as a reverse control. All observations,
including the slower first baseline, remain in the report.

| Artifact | Host execution ms | Median ms |
| --- | --- | ---: |
| Chrome baseline | 11254.0 / 9383.0 / 9285.2 | 9383.0 |
| Chrome revised | 8281.3 / 8180.0 / 8045.6 | 8180.0 |

Median host-time reduction is **12.8%**, or **14.7% more fixed-work capacity**.
The first baseline is an outlier; a single before/after pair would overstate the
gain. All deterministic counters, PCM and final-frame checksums match the six
Chromium controls exactly. No universal timing gain across hosts is implied.

One additional normal-playback window per artifact uses installed Chrome 157:

| Measure | Chrome baseline | Chrome revised |
| --- | ---: | ---: |
| Wall / guest seconds | 23.366 / 23.370 | 23.358 / 23.360 |
| Mean host slice ms | 4.2470 | 3.8976 |
| Presented frames | 593 | 605 |
| Missing samples, overflow, producer drops | 0 | 0 |
| Queue range, source samples | 2400–2401 | 2400 |

Both windows read 105 IDE sectors, emit 1,548 FM key events and 910 sound IRQ
edges, and retain zero sequence/checksum errors. Production and consumption both
remain about 48 kHz. The slice observation is about 8.2% lower, but a single
window per artifact does not establish repeatability or a visible-transition gain.
As in pinned Chromium, the baseline already has no audio underruns in this case.

### Deployed-artifact control

The public site's app, emulator Worker and AudioWorklet JavaScript match this
checkout byte for byte. The fetched public Wasm is 537,340 bytes, SHA-256
`bf5e65e17ef1e0c4e5f4d02a454f9d77d43fac93a7eb052f9a52a258313e89bd`,
with Last-Modified 2026-11-07 18:37:22 GMT. It differs from the locally built
baseline, so an additional installed-Chrome window uses that exact public Wasm
with the local game, without changing or uploading the installed disk.

That window advances 23.360 guest seconds in 23.363 wall seconds, presents 591
frames, reads 105 IDE sectors, emits 1,552 FM key events and 910 sound IRQ edges,
and records zero underruns, overflow, drops and sequence/checksum errors. The
queue remains 2400–2401 source samples. This is a local playback control of the
public artifact, not a full production-site or later-game test. A stale
JavaScript asset or sustained PCM shortfall is not reproduced in this scenario.

## Regression validation

The existing README fixture preparation commands regenerated the independent
MIT boot probes and hash-checked FreeDOS, HimemX, DOS applications, PMD, CPULEVEL
and DEBUG inputs and their required matching sources. Proprietary game disks
were used only in the separate local investigation.

| Check | Result |
| --- | --- |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 10 passed |
| `cargo test --locked`, test opt-level 1 with debug assertions/overflow checks | 610 passed across 42 suites; native compiler Rust 1.94.0 |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` | Passed |
| `node --test tests/audio-worker.test.js tests/audio-worklet.test.js tests/render-worker.test.js` | 20 passed |
| `npm test -- --workers=4` | Release Wasm/build/publication audit passed; Worker tests passed; Chromium 257 passed, 3 existing skips, both serving roots, 4.7 minutes |
| `git diff --check` and `node --check tools/measure_doukyuusei2.mjs` | Passed |

The complete browser run includes independent ADV98 production audio, desktop
HimemX/XMS, graphics, disk, protected-mode, paging and VM86 regressions. Publication
auditing found 27 approved assets and no game media. `/roms/` is now explicitly
ignored to keep the supplied installation disks out of accidental commits.
The benchmark remains opt-in and introduces no CI download of this title.

## Limits

The target is the base game installed into the existing synthetic FreeDOS
desktop, not an original MS-DOS/NEC firmware environment. Special Disk content,
later gameplay, save/load and spoken-audio behavior are not established by this
investigation. The OPN/OPNA implementation remains approximate.

The measured Chrome gameplay observations maintained approximately real
time with changing FM notes and no additional sink underruns on this fast host.
Those short observations do not reproduce the user's sustained audio complaint
or demonstrate its complete resolution. Interpreter headroom is useful evidence
for constrained hosts, but it is not a universal transition-speed or audio-quality
claim. Subsequent Firefox measurements and further optimization are documented
in the [second evaluation](doukyuusei2-performance-round2.md). The existing independent
[Firefox performance limitation](runtime-performance.md#revised-production-measurements-review-follow-up)
remains relevant.
