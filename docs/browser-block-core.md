# Browser-first CPU block experiment — Issue #180

This experiment tests a replacement execution boundary: generated Wasm owns
registers, ordinary RAM accesses and instruction timing until the next event or
work limit. Rust resumes once per run. It is an opt-in Cargo feature and tooling,
outside the published application. It is not a complete replacement PC-98 core.

The frozen control is `62315e2c41f294f3eceafad478c9d24368c58f4f` (PR #177).
Its release Wasm SHA-256 is
`f4fb5d3bdb8c880363764cb7ffa52c1d937469c1255aadeb5768ef5fe130bacb`.
The proposed default-adoption threshold is at least 30% less host execution time
on the fixed-work game control, with correctness and cold-cost checks. A smaller
gain retains the experiment without switching the web runtime.

## Sources and architectural choices

[NP2kai's pinned portable CPU](https://github.com/AZO234/NP2kai/blob/5939e0c6d5985c4c08fc70f289a83290e5d3e6f7/i386c/ia32/cpu.c),
[memory paths](https://github.com/AZO234/NP2kai/blob/5939e0c6d5985c4c08fc70f289a83290e5d3e6f7/i386c/cpumem.c)
and [scheduler](https://github.com/AZO234/NP2kai/blob/5939e0c6d5985c4c08fc70f289a83290e5d3e6f7/nevent.c)
motivate continuous CPU execution within an event budget and a short ordinary-RAM
path. Its C implementation also provides a useful browser reference when compiled
with Emscripten; a native-versus-Wasm ratio alone cannot isolate browser overhead.

[v86's Wasm translator](https://github.com/copy/v86/blob/master/src/rust/jit.rs)
shows the relevance of register locals, larger compilation units and code-page
invalidation to browser execution. [QEMU's translator documentation](https://www.qemu.org/docs/master/devel/tcg.html)
explains state-specialized blocks and chaining, including the need to return to
interrupt evaluation after state changes that may unmask interrupts. These are
design references. This experiment copies no upstream CPU implementation.

| Concern | This prototype | Replacement-core direction |
| --- | --- | --- |
| CPU state | Eight low-word GPRs and FLAGS in Wasm locals | Lower supported 16/32-bit instructions to one shared IR; flush only at defined exits |
| RAM | Direct shared-linear-memory loads/stores below `0xa0000` | RAM fast paths plus explicit mapped-memory slow exits |
| Timing | Exact current uncached Ap instruction, bus and address-dependency costs | Lower cache/timing state with the CPU; preserve the current clock model |
| Events | Strict budget before the earliest device or slice deadline | Same budget contract across linked blocks |
| Control flow | Up to 16 decoded instructions; direct loop back to its own entry | Multiple basic blocks per Wasm module and guarded chaining |
| Code writes | Compare the entire code span at entry; exit before a self-code store | Code-page generations covering CPU, BIOS, DMA and loader writes |
| Compilation | One asynchronous module per offered block | Compile groups of hot blocks, measure startup and memory cost |
| Browser | Existing Worker and imported Wasm memory/table | Worker-owned machine; no native executable-memory JIT or mandatory shared-memory threads |

Implementing the right-hand column is future work. The current measurement tests
whether removing instruction callbacks alone justifies adopting this first tier.

## Execution and correctness contract

`block-experiment` adds a context after the existing JIT scratch layout. The normal
packet ABI and production compiler remain available. `tools/lib/block-jit.mjs`
owns a fresh cache, chooses the direct compiler for supported offers and keeps the
existing guarded compiler for other offers. Its generated function imports only
memory and a completion callback. It has no per-instruction callback or JavaScript
loop in the hot path. Use one JIT owner per core, and clear translations when
changing owners.

The direct path requires the Ap real-mode, 16-bit, uncached configuration
(`CR0.CD=1`), ordinary segment limits/access, conventional-RAM code, no TF, no
interrupt shadow, no pending deliverable IRQ and a deferred-device execution
scope. Stack transactions, shifts, I/O, protected/vm86/paged execution,
REP and other unsupported instructions retain the guarded tier or interpreter.
Changing cache/descriptor assumptions invalidates a direct plan so it can rewarm
into the guarded tier. Segment bases are loaded at entry, not embedded as stale
operand addresses.

Before each generated instruction, guards check the remaining work, a complete
16-bit operand span, its physical RAM span, a potential self-code store and the
maximum cost against the event budget. Each committed prefix is final: memory
stores are not speculatively replayed or rolled back. An excluded access exits
before its store or read; the original instruction performs its bus effects.
A zero-instruction run falls back once to guarantee progress.

The budget inverts the existing rational CPU-to-scheduler conversion, including
its carried remainder. The complete generated run ends strictly before an event.
Its single completion updates counters, the final instruction's timing metadata
and register-write dependency mask, advances the rational converter once, and
ticks the bus once. The original next unit handles an event boundary. No clock
ratio, guest instruction timing table or PCM production rate is reduced.

The conservative entry comparison detects changes to any instruction in the
span, including stores by another translated block. This is deliberately not the
final page-generation scheme: that scheme must cover every writer before it can
replace byte comparisons. The prototype likewise does not bypass the cached
timing model by pretending cache hits are free.

## Reproduce the focused checks

Prepare the legal fixtures and dependencies using the normal [README checks](../README.md#reproduce-the-checks).
Then run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
  CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true cargo test --locked --all-features
python3 tools/check_ram_blocks.py
npm test
```

`check_ram_blocks.py` builds a feature-enabled optimized Wasm into a temporary
file, runs the original synthetic guest conformance probes, and removes that file.
It does not write `web/core.wasm`. `npm test` builds the default release artifact
and runs its existing Node and actual-browser acceptance. CI uses both paths.

The new native checks independently verify the maximal safe event budget using
the forward converter, admission without CPU/fetch effects, and transition back
to the guarded timing mode. Actual-Wasm checks compare the interpreter with
generated loads/stores, ALU groups, branch outcomes, partial work/event exits,
register dependencies, segment overrides, mapped/boundary operands, self-modifying
code and a mixed direct/guarded owner. Comparisons include CPU/debug timing state,
all conventional RAM, rendered frames and PCM bits. Literal arithmetic and loop
expectations supplement differential checks.

## Private game and browser-reference reproduction

Keep media, installed disks, captures and raw reports outside Git. The probes use
an installed FreeDOS(98)/HimemX HDD containing `A:\NANPA2\NANPA2`; they do not
distribute it. All disk writes affect fresh in-memory copies. Use the same lawful
FONT.ROM and NP2kai configuration as the earlier comparison. Input scripts and
the stationary bell-text mask validate scene correspondence; matching a text
mask does not imply identical CPU work or hardware timing between emulators.

For the private A/B, build the feature Wasm to an external file with the same
SIMD release flags and Binaryen `-O3 -g` as the default build. Preserve the frozen
control and then run:

```sh
node tools/measure_ram_blocks.mjs \
  --baseline=/absolute/control/core.wasm --candidate=/absolute/experiment/block.wasm \
  --media=/absolute/private/installed.hdi --repeat=3 \
  --output=/absolute/private/block-pairs.json
```

This paired probe alternates execution order every 10 ms of guest time. Each pair
boots fresh machines, settles asynchronous compilation, requires identical setup
state and checks CPU state, clocks, full-RAM hash, frame hash and PCM hash at 8,
16 and 24 seconds of scripted scene progression. Its measured active time excludes
those checkpoint comparisons and asynchronous compilation waits. `setupActiveMs`
and generated module statistics report cold work separately; `compilationMs`
includes scheduling delay and other work while a compile is pending, not isolated
compiler CPU time. This is a fixed-work core probe, not real-time audio playback.

The NP2kai browser reference needs an existing upstream Git clone, Emscripten and
make. No install or download is performed by the builder:

```sh
python3 tools/build_np2kai_browser.py --source=/absolute/NP2kai \
  --output=/absolute/new-reference --jobs=4
node tools/measure_np2kai_browser.mjs --build=/absolute/new-reference \
  --media=/absolute/private/installed.hdi --font=/absolute/private/font.rom \
  --config=/absolute/private/np2kai.cfg --output=/absolute/private/new-reference-run
```

The builder archives revision `5939e0c6d5985c4c08fc70f289a83290e5d3e6f7`
without changing the source clone. It adds missing portable ADPCM sources,
read-only guest clock/rate observers and standalone libretro VFS helpers, and sets
48 kHz / 851 frames per 56.4 Hz video frame. It uses portable i386 C, no native x86
assembly, no asynchronous CPU and no threads. The frontend requests Intel i486DX,
65,894,400 Hz (1.9968 MHz × 33), 4 MiB and PC-9801-86 sound. The local trial used
Emscripten 6.0.13-git and `-O2` for its frontend/link. Original upstream licensing
applies to that external build; neither its sources nor binaries ship in this app.

The reference runs 24 guest seconds of the stationary prologue bell scene with
software RGB565 frame copies and synthesized 48 kHz audio consumed by a counter.
GPU presentation and audio-device output are excluded. `--diagnostic=1` counts
nonzero PCM samples/energy as a separate diagnostic run; exclude that run from
performance statistics. CPU process time is unavailable here and is not reported
as a misleading zero. The roughly 0.16% CPU-clock difference, i486DX versus DX2
configuration, event models, render paths and per-frame time rounding remain
limitations even in the same browser. Do not compare this stationary scene
directly with the progressing-scene A/B above.

The matching stationary control for our two cores also records complete cold
setup, including core loading, JIT compilation waits, input scripting and setup
rasterization. Run fresh processes three times per core, alternating order:

```sh
node tools/measure_browser_bell.mjs --wasm=/absolute/control/core.wasm \
  --media=/absolute/private/installed.hdi --output=/absolute/private/new-control
node tools/measure_browser_bell.mjs --wasm=/absolute/experiment/block.wasm \
  --block=1 --media=/absolute/private/installed.hdi \
  --output=/absolute/private/new-candidate
```

## Measured result and adoption decision

The [sanitized measurements](browser-block-core-results.json) were collected on
an Apple M4 Max, macOS 27.2, ARM64, Google Chrome 157.0.8103.52. CPU-heavy validation
and other benchmark probes were not run concurrently with the selected timings.
Each listed median uses three fresh runs. The initial screen, separate PCM
diagnostic and first NP2kai run overlapping fixture preparation are excluded.

| Fixed guest work | Frozen current engine | Block experiment | Reduction |
| --- | ---: | ---: | ---: |
| 24.00028 s, progressing scene, paired active CPU/PCM probe | 5,400.3 ms | 5,041.0 ms | 6.65% |
| 24 s, stationary bell scene, active CPU + software raster | 5,052.6 ms | 4,643.5 ms | 8.10% |
| Same bell run, elapsed measurement loop including JIT waits | 5,140.6 ms | 4,777.9 ms | 7.06% |
| Cold setup through bell-scene readiness | 29,705.8 ms | 32,454.9 ms | **9.25% increase** |

The paired progressing runs each matched CPU/timing state, conventional-RAM hash,
frame hash and PCM hash/count at setup and 8/16/24 guest seconds. The selected
prototype committed 39,502,175 of 118,663,329 measured instructions directly
(33.3%), using 22,030,040 completion calls: only 1.79 instructions per call. There
were 425,373 zero-prefix fallbacks. At the end of that probe it had generated
1,995,592 bytes of modules versus 1,178,683 bytes in the control, a 69.3% increase.
The independent stationary runs also show greater code volume and cold setup cost.

The pinned NP2kai Wasm reference took 1,406.3 / 1,427.8 / 1,436.4 ms for
24.014471442 guest seconds. Against the stationary current-engine median, its
guest-time-normalized host-time advantage is approximately **3.54×**. It produced
1,153,105 stereo audio frames at 48 kHz and 1,355 software frames. Our stationary
control produced 1,152,000 audio frames and 1,354 frames over 24 seconds. The
separate diagnostic counted 1,250,582 nonzero PCM samples, so the reference was
not just issuing silent callbacks. This is a scoped guest-throughput comparison,
with the hardware/time-model and renderer differences listed above, not an
instruction-for-instruction efficiency ratio or a real-time playback guarantee.

**Do not enable this prototype by default.** It misses the proposed 30% threshold
and increases cold cost. Retain the reference engine and the opt-in experiment.
The next useful redesign step is a larger multi-block translation unit with
guarded links: the current small-block boundary still returns too frequently.
Then evaluate code-page generations and wider RAM/ISA/cache coverage against the
same oracle before considering a runtime replacement. The results do not justify
a large unmeasured rewrite or changing guest clock ratios to improve a score.

## Validation evidence and limits

Local checks passed: 639 native tests with the feature enabled and debug/overflow
checks retained, Clippy across all targets/features, formatting, 14 focused
optimized-Wasm tests, all 56 default Node tests, 57 legal-fixture/check commands,
and the default release/publication audit (28 assets). The actual Chrome game
controls above exercised both the default and experimental owners. Normal
actual-browser application acceptance remains a mandatory PR check.

The default release has no `block_*` exports or experimental web imports. Its
Wasm executable code section is byte-identical to the frozen control; changed
panic-location data and symbol names make the complete file 574,690 rather than
574,687 bytes. The measured feature artifact is 582,149 bytes. No upstream
reference binary/source or private media/captures are published.

During focused test development, initial assertions incorrectly required every
run to fill its work cap and assumed removed plans remained allocated; they were
corrected to the existing event/cache lifecycle. A descriptor-transition check
exposed repeated direct-tier rejection and led to invalidation before code-span
admission. The segment-override probe also needed valid warm-up addresses rather
than an unintended word access at `0xffff`. All final focused checks pass without
weakening production assertions. The external reference's initial sandboxed link
could not write Emscripten's installed symbol cache; the scoped link then completed
with authorized cache access. Early harness load/syntax errors produced no accepted
timing samples. Full replacement coverage and other browsers remain future work;
this experiment does not resolve the broader playback Issues #172 or #146.
