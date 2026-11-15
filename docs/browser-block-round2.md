# Browser block performance, round 2 — Issue #182

This continues the [first experiment](browser-block-core.md) with successive
implementation and measurement iterations. The direct tier remains opt-in and
outside the published application. The guest CPU clock, instruction tables,
scheduler conversion and 48 kHz audio production are preserved.

## Changes evaluated

| Configuration | Added work | Exploratory reduction against frozen production |
| --- | --- | ---: |
| A: stack | Word PUSH/POP and near CALL/RET in generated Wasm | 18.22% |
| B: shifts | Single-bit shifts/rotates and supported immediate counts | 17.10% |
| C: deferred flags | Materialize pending arithmetic flags at reads/exits | 16.14% |
| D: traces | Follow direct CALL/JMP targets within the 16-instruction limit | 24.78% |
| E: LEA | Direct 16-bit effective-address computation | 25.86% |
| F: guarded returns | Continue into the caller when RET matches its decoded successor | 27.39% |

These are single-run exploration controls, not independent estimates of each
feature's causal contribution. Every run used the same progressing scene and
CPU/timing, conventional-RAM, frame and PCM equality checkpoints. B alone did not
show a convincing improvement over A, but permits traces to cross shift operations
and is included in the final combination tested separately. C passed differential
checks but increased generated code from 2,816,289 to 3,249,417 bytes without a
clear throughput gain; it was removed. F is the selected combination, followed
by separate repeated controls. See [sanitized results](browser-block-round2-results.json).

## Execution contract

The old decoder/guarded compiler keeps its opcode contract. The experimental
collector can follow constant near CALL/JMP targets, stopping at conditional
branches, unpaired returns, unsupported instructions, repeated targets or 16 decoded
instructions. A direct prefix stops before an unsupported descriptor instead of
turning a noncontiguous trace into a guarded plan. Cached timing and other modes
continue to use the existing tier or interpreter.

Each trace records its exact code spans. Admission validates every span after
materializing deferred device work and checking pending interrupts. Writes are
guarded against the bounding interval of all spans, including gaps: this may
conservatively exit for a harmless gap write, but does not miss a later code span.
Global code-page generations remain future work. Registers stay in Wasm locals
across traced branches; conditional/back-edge execution retains the existing
work/event budget. A matched inlined CALL/RET may continue into the caller. RET always uses the actual
stack word; if that differs from the predicted successor, it commits RET and exits
before any caller operation. No memory transaction is speculatively replayed.
The last committed operation index still determines timing
metadata, including early exits within a callee.

Stack accesses validate the complete word and ordinary hidden SS descriptor
before effects. PUSH SP uses its original value; POP SP applies the increment
before replacing SP with the popped value. A conventional-RAM RET peek validates
the destination against the ordinary CS/RAM range before changing SP/IP. Mapped
operands, wrapping words and self-code stores exit into the original instruction.

LEA is decoded only for direct offers. It computes a wrapped offset, charges the
existing address-dependency/fetch costs, preserves flags and does not validate or
read an operand segment or mapped memory. It cannot leak into the legacy generated
compiler when cached timing or unusual code descriptors require fallback.

## Reproduction and measurement scope

The frozen production control is `62315e2c41f294f3eceafad478c9d24368c58f4f`, whose
executable Wasm also underlies the preceding main runtime. The previous experiment
is commit `66778129a0ed96b44d485c437c40036d39203fea`. Freeze both its feature Wasm
and its two `tools/lib/block-*.mjs` files before changing sources. Build the new
feature Wasm externally with the normal SIMD release flags and Binaryen `-O3 -g`.

The paired probe defaults to production-versus-experiment. To compare the old
experiment with the new one without accidentally using the new compiler on both:

```sh
node tools/measure_ram_blocks.mjs \
  --baseline=/absolute/frozen/block.wasm --baselineBlock=1 \
  --baselineRuntime=/absolute/frozen/runtime \
  --candidate=/absolute/current/block.wasm \
  --candidateRuntime=/absolute/current/runtime \
  --media=/absolute/private/installed.hdi --repeat=3 \
  --output=/absolute/private/new-pairs.json
```

The existing active-time, setup, scheduling, hashing and private-output rules in
the first report apply. Runtime files and Wasm bytes are loaded into allowlisted
server assets before browser startup; later edits cannot mix source revisions.
Raw game state, PCM, captures and profiles stay outside Git. New standalone bell
controls can select frozen compiler files with `--blockRuntime=/absolute/runtime`.
The NP2kai portable Wasm revision/configuration and text-mask scene check are
unchanged. Hardware/timing/render-path differences still limit the reference to
guest-throughput comparisons; no instruction-efficiency equivalence is claimed.

## Validation

The focused suite grows from 14 to 23 optimized-Wasm tests. It covers stack aliases,
odd/boundary words, return targets, all shift groups and representative masked
counts, later-span mutation, trace prefixes and callee returns, mapped/segment-
independent LEA, writes to future code and conservative gap exits. Every comparison
includes CPU/debug timing, conventional RAM, rendered output and PCM bits. Native
checks add unusual SS admission and the direct-only LEA contract.

Two test-authoring assumptions were corrected: enabling cached timing must clear
both CD and NW, and a trace can stop before RET at an audio deadline. The independent
single-completion CALL/RET control now starts just after an observed audio event;
separate tests retain partial-event exits. No production assertion or timeout was
relaxed. The rejected deferred-flags candidate's focused checks passed before removal.

Local final checks passed: 642 native tests with debug/overflow checks retained,
23 optimized-Wasm tests, 56 default Node tests, all-target/all-feature Clippy,
formatting, 57 legal-fixture/check commands and the 28-asset default publication
audit. A fresh feature rebuild is byte-identical to the measured final artifact.
The final actual-browser application acceptance is recorded in the linked PR.

One stationary E run completed its measurement, anchor checks and result write,
then stalled during browser shutdown. Inspection found only the owned Node driver
remaining, with no browser children; that Node was terminated. Its pre-shutdown
measurement remains recorded with this qualification. The three measurement
drivers now bound the close promise itself, then bound a kill of only their owned
browser, rather than merely scheduling a kill while awaiting close indefinitely.
Final F runs completed with the corrected drivers. Shutdown time is outside all
reported performance windows.

Required checks remain native Rust, all-feature Clippy/formatting, the opt-in Wasm
check script, default release/Node/publication checks and actual-browser application
acceptance. The source Issue requires no post-merge verification. Broader playback
Issues #172/#146 and the proposed 30% default-adoption gate remain separate.

## Held-out result and adoption decision

The final runtime files and feature Wasm were frozen before these controls.
The host remained Apple M4 Max / ARM64 / Chrome 157.0.8103.52. CPU-heavy validation
and other benchmarks ran outside the selected measurement windows. The JSON
records all individual samples and executable/compiler hashes, without guest
registers, code, pixels or PCM.

| Control, three repetitions | Baseline median | Final F median | Reduction |
| --- | ---: | ---: | ---: |
| Progressing scene vs frozen production | 5,519.2 ms | 3,949.1 ms | **28.45%** |
| Progressing scene vs previous experiment | 5,149.0 ms | 3,999.4 ms | **22.33%** |
| Stationary bell scene, CPU + software raster | 5,071.3 ms | 3,452.1 ms | **31.93%** |
| Cold setup through stationary-scene readiness | 30,952.9 ms | 31,432.5 ms | **1.55% increase** |

The two progressing comparisons are separate alternating paired experiments,
which explains the small difference between their final-candidate medians. All
six pairs matched CPU/timing, conventional-RAM hash, frame hash and PCM hash/count
at setup and 8/16/24 guest seconds. The direct tier now handles approximately 76.2%
of measured instructions, versus 33.3% in round 1, and averages 2.84 instructions
per direct completion versus 1.79. The remaining runs are still short. Generated
modules total 3,357,182 bytes, compared with 1,178,683 for production and 1,995,592
for the old experiment; these totals are not resident-memory measurements.

Fresh NP2kai Wasm reference runs took 1,475.6 / 1,510.8 / 1,449.3 ms for
24.014471442 guest seconds. The stationary production/reference normalized
host-time ratio is 3.44×; the final prototype/reference ratio is **2.34×**.
The stationary controls used fresh processes; production/NP2kai samples were
collected during E evaluation and the final F samples followed its code freeze.
Keep the existing hardware, cycle-policy, raster and audio-device scope caveats
from round 1. This measures guest throughput, not equal retired instruction counts
between different emulator models or calibrated live playback.

**Retain the improvement in the opt-in experiment.** The stationary control now
exceeds the proposed 30% threshold, but the primary progressing control remains
below it. Cold setup and generated code volume also increase. The published
runtime therefore remains the existing engine. Further iterations should target
the remaining short run boundaries, unsupported work and compilation/code volume,
while preserving the same clock/event/PCM oracle. A broader replacement remains
unproven; these measurements establish incremental progress toward the reference.
