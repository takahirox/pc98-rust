# Calibrated CPU-budget playback — Issue #171

This is an opt-in reproduction workflow for the low-frame-rate/audio complaint
tracked in #146. It measures the production scheduler, independent renderer and
AudioWorklet under a shared CPU budget. It does not change emulation or establish
60 fps on a particular older PC.

## Why page throttling is insufficient

On the development host (Apple M4 Max, macOS 27.2, stock Chrome
157.0.8103.39), a separate synthetic Wasm probe with CDP page throttling at 4x
measured 22.6 to 94.3 ms on the page (4.17x), but 22.7 to 23.2 ms and 22.6 to
23.0 ms in two dedicated Workers (about 1.02x each). These are diagnostic probe
results, not playback timings. The emulator runs in a Worker, so a page-only
setting is not evidence of a slower emulator CPU. Enabling the Wasm debugger is
also unsuitable; see the [earlier reproduction investigation](doukyuusei2-reproduction.md).

## Run the controls

Prepare the normal release build and legal public fixtures using README. Supply
an existing installed game HDD outside every checkout. Docker must already be
running with the cached `mcr.microsoft.com/playwright:v1.56.1-noble` image; the
tool does not install software, pull images or change Docker settings. The image
contains Chromium; no Playwright package or driver is used.

```sh
node tools/measure_cpu_budget.mjs --media=/tmp/pc98-146/installed.hdi --output=/tmp/pc98-evidence/matrix.json --budgets=unlimited,1,0.75,0.5 --repeat=2 --clicks=24
```

Create the output's parent directory first. Media and output must be absolute
paths outside every Git worktree, with no commas/newlines in resolved bind paths.
An existing output is rejected. Keep private output on local storage. The default
budgets are `unlimited,1,0.5,0.25`; supported numeric budgets are 0.25 to 2 CPUs,
with whole-microsecond quotas. Start with `unlimited`. Repeats are bounded to 1–3;
the second repeat reverses constrained-budget order after its unrestricted control.
Run one evaluation at a time, without builds, other benchmarks or profilers.

The host wrapper checks the publication allowlist, freezes the approved web tree
and measurement scripts, hashes the media, records the cached image's immutable
ID, and launches only its own bounded containers. Each has two allowed CPUs,
2 GiB RAM, 256 MiB shared memory and no external network. A 4 ms quota period
gives `cpu.max` values of `max 4000`, `4000 4000`, `3000 4000` and `2000 4000`
for the example above. Requested, configured and observed controls are retained
and checked. Only frozen public assets/scripts and the single private HDD are
mounted read-only; the private evidence directory is writable. No Docker socket
or host profile is mounted. The local server and browser communicate over
container loopback. The wrapper removes each owned container, including on
failure, interruption or timeout; cleanup failures retain the exact owned ID.

The [Docker resource documentation](https://docs.docker.com/engine/containers/resource_constraints/)
describes CPU quotas. The fixed period avoids multiplying the usual 100 ms
quota-period stall into playback. Linux requires a minimum 1 ms quota; see
[CFS bandwidth control](https://docs.kernel.org/scheduler/sched-bwc.html).
Quota scheduling still creates periodic stalls: it does not uniformly slow each
instruction or reproduce an old CPU's caches, instruction mix, memory or GPU.

Each container first runs `calibrate_cpu_budget.mjs`: an unchanged Wasm affine
recurrence in the page and two dedicated Workers, warmed and measured nine times
per actor with rotating order. An independently calculated modular result checks
every computation. Measurements are sequential, so a one-CPU budget need not
slow one busy thread. Sub-core budgets must materially slow both Workers against
the unrestricted control in the same repeat, or the row is invalid. Report actual
ratios; do not relabel a quota as a measured slowdown.

Playback then uses [the ordinary reproduction tool](doukyuusei2-reproduction.md)
with a fresh headless Chromium profile, explicit container-only `--noSandbox=1`,
JIT enabled, audio enabled from boot, normal guest disk/input paths and unchanged
Worker/Wasm/worklet bytes. No CDP, debugger, profiler, manual guest advancement or
PCM discard is used. The same wall-paced 48 prologue clicks, menu inputs and 24
flashback clicks run at every budget. Slower playback can process fewer inputs
or reach different dialogue states; this is a production response comparison,
not identical guest work. Faulted, hidden, paused, flushed or too-short phase
windows are invalid, not healthy throughput.

## Interpret the evidence

The matrix report contains runtime/tool hashes, image identity, controls, actual
calibration ratios, validity and phase summaries. The private adjacent directory
contains frozen sources and full calibration/playback rows, including cgroup
`cpu.stat` before/after, browser user agent, guest work counters, JIT counters,
slice histograms, render/presentation costs and input times. Do not commit the
media, raw reports, profiles, pixels, PCM or guest code. Only sanitized aggregates
belong in repository documentation.

`valid` describes measurement validity, not successful performance. A valid run
may have very low fps and substantial missing PCM. Compare each phase against
its unrestricted control and keep startup errors separate from phase deltas.

- Guest seconds per wall second detect a producer deficit. Presented fps alone
  can hide a slow guest or count repeated presentations of a static image.
- Missing samples (`underflows`) count **device-rate frames**. Worklet
  `scheduledSamples` counts actual PCM output; add the missing frames to measure
  callback-clock progress relative to wall time. Source production/consumption
  counters use 48 kHz and must not be mixed with device-rate counters. Overflow,
  dropped-sample, sequence and checksum counters are separate.
- Guest-slice, snapshot, raster and copy costs are elapsed host times. Under CFS
  they include descheduling. Parallel Worker elapsed times are not additive CPU
  shares. JIT `compilationMs` is asynchronous wall latency, not compiler CPU time.
- `translatedFraction` compares retired translated instructions with completed
  instructions over the same phase. REP elements have their own counter. A high
  translated fraction alone does not prove that JIT callbacks are inexpensive.
- Headless/Linux software rendering and absent device-output statistics limit
  conclusions about an actual display or DAC. Callback starvation is observable;
  synthesis fidelity and the hardware's final audible output remain separate.

Primary matrix runs disable Canvas patch readbacks (`--samplePatches=0`, default)
to avoid GPU readback affecting fps. Repeat selected controls with
`--samplePatches=1` to record nine small CG-area fingerprints at 250 ms intervals.
Compare disk-read changes and fingerprint changes around input events, and report
the sampling interval and possible readback overhead. These detect visible image
responses; they do not identify the exact instant a full scene finishes loading.

## Results and next implementation

Controls use main `3a30053`, unchanged release Wasm SHA-256
`6d9d2c74eb9bb56278dd041c224a3535ce016fda3bd61077d16c5838531aea57`
and `jit.js` SHA-256
`351075c346708ce72d5776cf3b2a4cff055c08c954baee5d31bcaf5ea084b836`.
The cached arm64 image ID is
`sha256:f1e7e01021efd65dd1a2c56064be399f3e4de00fd021ac561325f2bfbb2b837a`;
inside it, Linux 6.12.54-linuxkit, Node 22.20.0 and HeadlessChrome 141.0.0.0
run on the M4 Max development host. The reduced browser user agent says x86_64;
the actual image and Node architecture are arm64. This is not the host's stock
Chrome 157 and not an actual old PC.

### Repeated budget controls, November 12, 2026

Eight controls use the frozen tooling above, two per budget, without patch
readbacks. All eight pass capture/calibration validity checks, preserve runtime
identities and finish without emulator/render faults, overflows, dropped samples,
sequence errors or checksum errors. Both Workers materially slow at sub-core
budgets. The following are ranges across the two runs, not confidence intervals:

| CPU budget | Page slowdown | Worker A slowdown | Worker B slowdown |
| --- | ---: | ---: | ---: |
| Unrestricted | 1.00 | 1.00 | 1.00 |
| 1 | 0.99–1.04 | 0.99–1.05 | 0.98–1.05 |
| 0.75 | 1.40–1.44 | 1.32–1.40 | 1.31–1.45 |
| 0.5 | 2.14–2.30 | 2.11–2.44 | 2.06–2.45 |

The first input window (about 29 wall seconds) performs another 105 HDD reads in
every control. It provides the clearest comparable response boundary:

| CPU budget | Guest s / wall s | Presented fps | Missing device frames | Mean guest-slice ms | Mean raster ms | Mean copy ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Unrestricted | 0.992–1.000 | 59.20–60.04 | 0 | 2.96–3.68 | 0.37–0.45 | 0.075–0.092 |
| 1 | 1.000 | 59.96–60.00 | 0 | 3.32–3.36 | 0.40–0.42 | 0.076–0.079 |
| 0.75 | 0.911–0.968 | 34.48–47.32 | 24,270–87,727 | 5.26–7.81 | 0.69–1.06 | 0.092–0.168 |
| 0.5 | 0.665–0.686 | 26.34–26.77 | 432,021–471,925 | 11.45–11.56 | 1.53–1.59 | 0.155–0.212 |

This establishes **0.75 as the first sampled failing budget below 1**, not the
precise minimum capacity for 60 fps. The one-CPU controls have 632/1,535 missing
device frames over the whole boot-to-end trace, outside these two phase deltas;
they are not claimed to have perfectly continuous startup. Unrestricted controls
have zero missing frames throughout. Unrestricted repeat 2's guest and callback
clock ratios both dip to 0.992 in the first window without missing queued PCM.

At 0.5 CPU, callback-clock progress is 0.992–0.995 in the first window, while
guest progress is only 0.665–0.686. The second input window falls to 8.38–9.48 fps,
0.440–0.449 guest/wall and another 393,596–406,652 missing frames, despite
0.996–0.997 callback-clock progress. The producer is substantially behind the
sink. Raster/copy costs also rise under contention, but lowering transfer cost
alone cannot explain or repair this guest-time deficit. The first window's mean
snapshot cost remains 0.030–0.052 ms at this budget.

At 0.75 CPU, callback-clock progress is 0.973–0.985 in the first window: scheduling
also contributes, and not all elapsed slowdown can be assigned to guest compute.
Both traces end at 973 HDD reads, with no further disk reads in the second input
window, rather than the 1,030 reads reached by unrestricted, one-CPU and 0.5-CPU
controls. The `flashback` phase label marks attempted inputs, not verified guest
scene state. Its roughly 60 fps at 0.75 CPU is **not** recovery on the same
flashback workload. Do not average that interval into a claim of successful
scene-change performance. Throttled-period deltas are nonzero in constrained
controls and zero in unrestricted controls.

### Image response controls

Two further controls enable patch readbacks, with the same frozen tools/runtime
and 48/24 input sequence. Both pass capture/calibration validity, with no
overflow/drop/sequence/checksum errors. Actual Worker slowdowns are 1.72x/1.72x
at 0.5 CPU. Times below are wall seconds since the prologue input-window marker;
each event changes all nine sampled patches:

| Observation | Unrestricted time / HDD reads | 0.5-CPU time / HDD reads |
| --- | --- | --- |
| First large patch change | 8.064 / 931 | 8.559 / 934 |
| Next large patch change | 11.314 / 934 | 11.565 / 934 |
| Next large patch change | 14.818 / 973 | 14.817 / 973 |
| Later distinct patch vector | 41.563 / 1,030 | Not observed; trace ends at 973 reads |

The adjacent sampling gaps are about 247–268 ms. Patch vectors can recur during
fades or other display states; these observations do **not** establish a uniform
image-load slowdown or exact input-to-completed-CG latency. The final distinct
vector from the unrestricted trace is absent from the constrained trace, which
also performs no further reads in its second input window. This supports a
progress/response divergence, not an identical-scene latency comparison.

With readbacks, the first window measures 57.72 fps/1,064 missing device frames
unrestricted and 27.81 fps/449,429 missing frames at 0.5 CPU. The primary controls
without readbacks measure 59.20–60.04/zero and 26.34–26.77/432,021–471,925
respectively. Observer work and trial variation are not separated by these two
supplemental traces; no zero-overhead claim is made. They corroborate low fps and
missing PCM under the quota without replacing the primary performance table.

### Next optimization

Unrestricted JIT coverage is 80.0–80.5% of completed instructions in the first
window and about 78.3% in the second. Yet batches average only 2.31–2.36 retired
instructions per callback: the first window alone makes 26.9–27.5 million batch
callbacks, with additional unbatched translated instructions. At 0.5 CPU, first
window coverage falls to 73.1–74.7% and batches average 2.28–2.31 instructions.
REP elements are only about 0.4% of completed-instruction count in unrestricted
first windows; count alone is not a timing profile, but it does not justify
prioritizing a broad REP rewrite.

Source inspection shows that generated runs stop before the next memory
operation and Rust still guards/retires every instruction. The next selected
experiment is therefore [reducing JIT instruction-boundary overhead (#172)](https://github.com/takahirox/pc98-rust/issues/172):
measure that hot path separately, then investigate generated conventional-RAM
runs and safe guard hoisting. Preserve exact device events, IRQs, timing,
dependencies and self-modification; reject unproven speedups or speculative
visible stores. This is a selected experiment, not a measured implementation
gain. Asynchronous compilation latency grows under quotas, but these results
do not measure compiler CPU time and do not establish compilation as the cause.

This change adds evaluation tooling, not an emulator optimization. The symptoms
are reproduced under a calibrated CPU budget; actual old-core/GPU equivalence
and real low-end 60 fps remain unproven. Issue #146 and implementation #172 stay
open.

## Validation

All four measurement-module syntax checks pass. The 54 Node tests pass,
including three new fast tests for quota validation, independent synthetic Wasm
results, and playback counter units/invalid windows. Six invalid CLI/path cases
are rejected before browser/container startup. The eight repeated primary and
two image-response controls pass capture validity; their actual performance
failures are retained above. Current measurement-file hashes match the frozen
evidence, including host harness SHA-256
`7877eeb1cce39e2a046bc553ec774e8e27e41a8880d54e58baf2315b7501b512`.
Every owned evaluation container is removed after completion.

The publication audit passes with 28 approved assets and the unchanged
572,042-byte Wasm. Relative documentation links and whitespace are checked.
Native tests, a new Wasm build and local full browser acceptance are not repeated
for this tooling-only change; emulator/product assets are unchanged. The
existing PR workflow still performs its normal acceptance suite. No new heavy
resource matrix runs in normal CI.

Required post-merge verification: none.
