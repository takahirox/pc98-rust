# Issue #140 validation — 2026-11-06

The [timing contract](cpu-timing.md) documents the selected CPUs, reference tables,
clock conversion, board policies and remaining accuracy limits. This record
covers the implementation in this worktree; it contains no physical-hardware
measurement or claim that a commercial game's transition problem is resolved.

Environment: macOS/Apple Silicon, Rust 1.94.0, Node 26.12.1, NASM 3.02,
Playwright 1.56.1 Chromium, native host and release `wasm32-unknown-unknown`.

## Checks

| Check | Result |
| --- | --- |
| README fixture preparation, including pinned FreeDOS binaries/sources, DOS applications, CPU, HDD, desktop and independent boot probes | Passed |
| `python3 -m unittest discover -s tools -p test_fixture_fetch.py` | 10 passed |
| Strict preparation of all 12 revised fixture manifests, without `--update-manifest` | Passed; source, assembly, image and result contracts verified |
| `CARGO_PROFILE_TEST_OPT_LEVEL=1 cargo test --locked --no-fail-fast` | 579 passed, zero failed/ignored, across 41 test targets including doc tests |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo fmt --check` and `git diff --check` | Passed |
| `npm test -- --workers=4` | Release Wasm build, font/media/site packaging checks passed; 12 Node audio tests passed; 245 Chromium tests passed |

The three Chromium skips are the existing `/pc98-rust/` duplicates for the
1 GiB high-sector, 512 MiB DOS, and occupied 512 MiB resource scenarios. Their
`local` counterparts passed; no new skip was added. All other browser tests ran
at both serving roots.

Native guest validation used test-profile optimization level 1, retaining debug
assertions and overflow checks, to avoid repeatedly paying the unoptimized
interpreter's cost for the greater instruction throughput now represented by
DOS time waits. An earlier ordinary unoptimized run exposed timing-dependent
fixture assumptions; those failures were corrected and the complete optimized
suite was then run successfully. This record does not claim a final complete
unoptimized run.

The native timing target contains 11 tests with literal NEC V30 and Intel DX2
clock expectations. It covers register/memory/immediate forms, stack and
control transfers, taken/untaken branches, shift counts, system instructions,
REP lengths/setup/early termination/interrupt restart, alignment, VRAM, I/O,
cache enabled/disabled and invalidation, explicit interrupt entry, address
dependencies, fault retirement, idle time and fractional clock conversion.
A one-second device-time check runs different CPU instruction mixes on both
profiles and requires 48,000 samples and the configured 1,996,800 Hz PIT phase.
Firmware waiting time is checked against total device time without a second tick.

## Workloads and elapsed time

The deterministic planar-transfer test fills one complete 32 KiB plane with
16,384 aligned `REP STOSW` elements. The result is compared to the documented
instruction formula plus the separately stated video wait policy:

| Profile | Modeled core clocks | Guest duration |
| --- | --- | --- |
| V30-10 | 98,311 | 9.8310 ms |
| DX2-66 | 196,619 | 2.9790 ms |

These numbers validate the implemented table/policy arithmetic, including REP
setup once and the cold DX2 instruction read. They are not measured PC-98 VRAM
bandwidth. No contention, V30 prefetch depletion or DX2 write-buffer overlap is
inferred from this comparison.

The production worker's legal 98VIDEOP workload completed boot, image/video
decode, playback and return to DOS. Diagnostic markers observed 3.090030375
seconds of guest time; wall time was 3.0825 seconds at `/` and 3.086 seconds at
`/pc98-rust/`. This is a complete-workload observation rather than an isolated
decoder benchmark or a hardware acceptance threshold. Guest and host durations
remain separately visible through the diagnostic API.

The unchanged ADV98/PMD music and scene workload passed the production audio
continuity test with a 48 kHz sink at `/` and a 44.1 kHz sink at `/pc98-rust/`.
Across 16 approximately one-second windows on each path:

- Missing samples were zero in every window.
- Source/rendered sample differences were zero at 48 kHz and at most 1.089
  source samples at 44.1 kHz; guest-time/source agreement stayed within the
  existing two-sample bound.
- Sequence/checksum/drop/overflow checks, FM/SSG pitch changes, PMD tempo/key
  activity, sound/PIT IRQ activity, scene changes and disk reads passed.
- Pause, mute, AudioContext suspend/resume, reset, visibility and credit-return
  behavior passed without unbounded catch-up.

The channel-3 guest retains its existing pitch tolerance of
`max(0.03 Hz, 0.02%)` and at least 3,500 samples per stage, cold and after reset.
Stages now wait for six vertical periods instead of assuming that 80,000 LOOP
instructions always span 80 ms.

## Timing-dependent probe corrections

- Integration boot/input waits now specify device time; instruction tracing
  still steps exactly one CPU unit. Historical slot-count limits that represented
  elapsed time were converted to device-clock limits.
- PIT probes use otherwise unused channel 2 for device-clock delays. A complete
  initial read pair lets the programmed counter load before the first latch;
  the faster Ap exposed the prior-count snapshot permitted before that clock.
  Literal peripheral clock traces and IRQ/result/guard assertions remain intact.
- Counter-latch raw low bytes vary with CPU instruction latency. Both hosts now
  verify the retained high byte, fresh-count interval, minimum elapsed count
  and every guard; the fixture manifest explicitly masks raw observation fields.
- FDC byte ISRs read PIC ISR state on the separate result IRQ, rather than
  spending that time on every data byte. Actual 500-kbit/s deadlines, data/result
  IRQ counts, payloads, EOI checks and full media equality remain asserted.
- RTC hold and VM86 callback probes wait in the peripheral clock domain instead
  of using LOOP counts as time. Architectural-state snapshot tests exclude the
  new elapsed-time counters while retaining their previous architectural checks.

## Limits

No target machine, hardware trace or physical audio recording was available.
The existing BIOS-advertised 8 MHz-family PIT configuration remains an explicit
board assumption even with the selected 10/66 MHz CPU cores; alternative system
oscillators are documented in the timing contract. Board waits, oscillator
coupling, V30 prefetch, detailed cache/pipeline/write-buffer behavior and synthetic
BIOS/LIO execution latency remain approximations or unmodeled effects. Hardware
comparison and diagnosis of a particular game's slow transition remain unverified.
No post-merge-only check is required. The initial checks above preceded publication;
the continuation results below record subsequent publication and CI evidence.

## Continuation validation

The continuation published the preserved INT/POLL corrections, then corrected
V30 divide-error timing against NEC table 6-1. Unsigned errors use 45 clocks;
signed errors use the documented 45–55 range's upper bound. Successful division
and external INT costs are not added to these complete exception totals.
All 16 timing tests and 15 library tests passed with test optimization level 1,
as did Clippy, formatting, whitespace and strict FDC BIOS fixture preparation.

The [GitHub run for `70e8f95`](https://github.com/takahirox/pc98-rust/actions/runs/42228222454)
completed all 590 native tests across 41 targets with no failures or ignored tests
in the default unoptimized profile. That native step took 42 minutes 24 seconds.
Release Wasm/build/packaging and 12 Node audio tests also passed, but the job hit
its 45-minute limit during Chromium tests. That cancelled run is not a complete
acceptance pass.

CI and the README reproduction commands now use test optimization level 1 to
reduce host interpreter cost for guest-time workloads. Debug assertions and
overflow checks are explicitly enabled; test selection, guest-time budgets,
assertions, browser checks and the job timeout are retained. This changes host
validation throughput, not emulated clock rates or guest behavior.

The [next GitHub run](https://github.com/takahirox/pc98-rust/actions/runs/42235234785)
passed native validation and 243 Chromium cases, with the three existing skips.
Both production-audio cases failed while running alongside accelerated guest
probes: observed production was approximately 35,000–38,000 source samples per
second, below the 48,000-sample playback demand. These are real underruns, not a
passing continuity result. CI now schedules the two wall-clock audio cases
sequentially before the functional browser probes, reusing the release Wasm build
and fixtures. Both serving roots/sample rates and every continuity, pitch, tempo,
scene-change and transport assertion remain required. The application, its clock
rates, audio reserve and work caps are unchanged by this scheduling adjustment.

The [isolated-audio run](https://github.com/takahirox/pc98-rust/actions/runs/42240559688)
also failed both audio cases, producing about 40,000 source samples/second.
Isolation alone was insufficient. Trace counters show 3.2–3.4 million execution
units/second, while the steady PMD workload requires about 4.16 million at the
selected CPU rate. A Node/V8 profile of the actual release Wasm core identified
CPU rollback cloning and repeated address checks as major host costs.

The host execution changes keep empty TLB storage absent, copy populated storage
on mutation, compare register words without repacking, and inline ordinary
address/timing paths while keeping page walks separate. A sampled run of five
guest seconds of the same PMD workload took 3.04 host seconds before and 2.30
after on the macOS host. Both executions consumed exactly 20,812,019 units and
330,000,015 core clocks, advanced 5.00000025 guest seconds and ended at 172 FM key
edges. These are host-throughput observations, not target-hardware measurements.

Local validation of this change passed 127 native tests across the library,
timing, protected-mode, paging and VM86 targets, Clippy, formatting, whitespace,
strict preparation of four relevant native fixtures and release Wasm/site
packaging checks. Both actual Chromium production-audio cases passed at 48 kHz
and 44.1 kHz, with zero missed samples in all observation windows and the existing
pitch/tempo, scene, IRQ, resampling and transport assertions retained. These local
checks do not turn either failed GitHub run into passing acceptance evidence.

The Linux [Worker-profile run](https://github.com/takahirox/pc98-rust/actions/runs/42253767204)
passed native validation but still failed audio continuity. Its first windows
produced about 47,000–49,000 samples with 2,326/4,379 missed device frames.
Failure-only sampling found substantial frame-rendering work alongside CPU
execution and idle time. These samples are diagnostic observations after failure,
not a hardware comparison or proof of sustained playback.

Scheduled host presentation now lets the existing PCM reserve refill first;
an overdue preview is still attempted after at most 100 ms of deferral, plus
one bounded task. Frame-buffer acknowledgments cannot bypass this audio priority.
Guest display clocks, PCM reserve, execution caps and acceptance assertions are
unchanged. A deterministic scheduling test includes a 22 ms renderer and 7 ms
per 10 ms guest slice: audio stays continuous and preview intervals stay bounded.
Removing only the priority check reproduces underruns. All 13 Node audio tests
and both actual Chromium audio cases passed locally, with zero missed samples
in every observation window at 48/44.1 kHz. Current-head CI remains required.

The [run on `d182500`](https://github.com/takahirox/pc98-rust/actions/runs/42255144788)
passed native validation but failed audio again (about 36,000–38,000 source
samples/second). Rendering priority alone did not resolve the CI host deficit.
Worker sampling still placed rollback cloning among the largest costs.

The next host change copies Copy state directly in the rollback checkpoint,
inlines fetch/timed-step boundaries, and avoids PIC arbitration without requests
and PIT counter updates without an input clock. A sampled five-guest-second
release-Wasm PMD workload took 2.28 seconds before and 1.97 after locally. Both
consumed 20,812,019 units/330,000,015 core clocks, advanced 5.00000025 guest
seconds and ended with 172 FM key edges. This host measurement does not predict
Linux runner performance. Address-helper and additional Machine inlining trials
showed no benefit and were discarded.

All 240 focused native tests across library/timing, protected mode, paging, VM86,
PIC and PIT passed, along with Clippy, format/whitespace, 13 Node audio tests,
the release Wasm build and 26-asset site packaging checks. An initial PIC attempt
lacked local fixture files; strict preparation from checked-in sources resolved
that setup failure before the successful run. No fixture contract was changed.
Both final local Chromium audio cases passed again, with zero missed samples in
all observation windows. Complete acceptance still depends on current-head CI.

## DX2 review corrections — 2026-11-07

The review regressions reproduced BOUND range faults at 33 execution clocks
instead of 50, byte memory RCL by nine at 17 instead of 16, and VM86 CLI faults
three clocks below the documented total. The DX2 data book, pp.10-12–13 and
table 10.1 notes 4–5, supplies the literal expectations for these corrections.
The original instruction fault now selects BOUND/VM86 exception overheads;
entry, descriptor loads and physical bus costs remain separate. Carry rotates
above operand width share the documented register/memory formula.

Validation in this fix worktree:

- `CARGO_PROFILE_TEST_OPT_LEVEL=1 cargo test --locked --test cpu_timing
  --test vm86 --test protected_mode --test paging`: 114 passed. New cases cover
  BOUND inside/outside its bounds, 16/32-bit operands, restart/retirement and
  separate bus costs, VM86 flag/control/I/O exceptions with accessed/unaccessed
  descriptors, and RCL/RCR register/memory forms with immediate/CL counts around
  8/16/32-bit boundaries and count masking.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check`: passed.
- Release `wasm32-unknown-unknown` build, media/font checks and
  `npm run check:site`: passed; 26 approved publication assets.
- `node --test tests/audio-worker.test.js tests/audio-worklet.test.js`:
  13 passed.
- `npx playwright test tests/browser/vm86.spec.js tests/browser/paging.spec.js
  tests/browser/protected-mode.spec.js tests/browser/v30-bound.spec.js
  tests/browser/worker-runtime.spec.js --workers=2`: 16 passed initially;
  the two DOS video worker cases lacked their local fixture. After strict
  `tools/prepare_dos_apps.py` preparation, rerunning only those two cases with
  `--grep 'Worker progresses through DOS graphics and PCM' --workers=1` passed
  at both serving roots. All 18 selected cases therefore passed.

Missing native fixture files were prepared from checked-in sources before the
successful native run. Native tests used Homebrew Rust 1.94.0; the Wasm build
used rustup Rust 1.99.0 as prescribed by `tools/build_web.py`, after the system
compiler reported that its Wasm target was absent. No fixture contract or
acceptance assertion changed. These are focused checks of the revised tree;
the inherited full CI result belongs to upstream PR head `8c4ac87`, and
publication/current-head CI remain downstream work.
