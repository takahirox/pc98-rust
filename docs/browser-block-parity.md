# Browser throughput parity with the pinned PC98 reference

This continues [round 5](browser-block-round5.md) for Issue #193 after importing
main's CRT presentation and fullscreen changes. The selected browser runtime
learns frequent forward branch successors and lowers a proven read-only BIOS
keyboard query into an event-bounded Wasm transaction. The guest still executes
and accounts for INT, the firmware OUT and IRET.

## Repeated results

Three fresh runs per retained control give these medians:

| Control | Frozen main | Selected | Host time reduction |
| --- | ---: | ---: | ---: |
| Stationary scene, CPU and software raster | 2,923.4 ms | 1,311.8 ms | **55.13%** |
| Progressing scene, active work | 3,836.9 ms | 2,335.2 ms | **39.14%** |
| Stationary cold setup | 32,293.2 ms | 30,980.4 ms | **4.07%** |

Fresh pinned NP2kai runs have a median of 1,460.9 ms for 24.014471442 guest
seconds; our stationary windows advance exactly 24 guest seconds. Normalizing by
guest time, the selected/reference host-time ratio is **0.898**, versus **2.002**
for main. This meets Issue #193's ratio target of at most 1.05. All three selected
stationary runs take 1,287.5–1,317.9 ms. The comparison's CPU/timing/render models
and workload scope are described below.

Every progressing pair improves by 38.80–39.63%, with matching CPU/time, all
conventional RAM, frame and PCM at setup and 8/16/24 guest seconds. Each window
advances 24.0002795 guest seconds. These are medians of three samples, not
confidence intervals. Cold setup ranges overlap; these samples do not establish
a substantial startup improvement.

The [sanitized numeric results](browser-block-parity-results.json) contain sample
times, artifact/runtime identities and resource counts. The selected normal-build
core is 590,414 bytes, 2,601 bytes larger than main. Progressing generated code
grows from 4,747,747 to 5,408,439 bytes across 3,437 and 3,618 modules. The bounded
choice table reserves 256 KiB; additional per-plan fields add at most 128 KiB
across 4,096 slots, and the completion context grows by 24 bytes. Generated-byte
totals are not resident-memory measurements.

Progressing direct completions fall from 26,707,703 to 15,143,698, while direct
instructions rise from 91,056,703 to 106,054,200. Zero-unit direct exits rise from
1,516,730 to 1,936,475 as the three-unit admission declines near work/event
boundaries; the original path executes those partial prefixes. All retained
runs have zero compiler failures.

## Selected design

Forward conditionals initially follow their fallthrough. A trace with at least
32 taken side exits comprising at least 75% of completed runs is invalidated and
rewarmed with the taken successor. The generated branch keeps its actual-IP
check; a later fallthrough exits before executing the predicted taken path.
The bounded choice table keys the physical CS base and branch IP, checks the
current decoded target, and clears on JIT reset. This changes code selection,
not the architectural branch outcome.

The direct tier recognizes a software INT whose current IVT entry points to the
standard synthetic keyboard thunk F000:1060, with AH=04 and ROM bytes E6 F0 CF.
Selection uses a conservative AH hint; runtime guards remain authoritative. The
transaction requires Ap real mode, uncached timing, an ordinary real CS, CPL0,
an ordinary 16-bit SS, no pending firmware request, at least three remaining
work units, and enough clocks strictly before the earliest device event. Paging,
protected mode, traps, interrupt shadows and pending IRQs retain existing entry
guards. Other services and unproven calls retain the original execution path.

Before any push, generated Wasm checks the current IDT limit/base/vector, ROM,
AH, stack span and the entire trace's code hull. It reads the vector before
writing FLAGS, CS and return IP in their original order, then reads the current
keyboard group. This preserves aliases between the saved frame, IVT and BIOS
bitmap. Mapped, wrapping or self-code frames exit before INT. A failed three-unit
budget also exits before INT, allowing the original path to expose INT, OUT or
IRET separately when the caller requests a partial prefix.

Execution latency comes from the shared instruction tables. Uncached word beats
retain odd and four-byte-crossing penalties; vector reads, all six stack word
transfers and all three instruction fetches are charged. The completion preserves
the last IRET's timing, fetch address, instruction length and SP write mask,
normalizes saved FLAGS, clears RF/AC as the original sequence does, and retains
the firmware cache flush. No device event can occur inside the transaction;
ordinary completion advances the unchanged rational clock converter and devices.
Observer and synchronous reference APIs continue to execute original instructions.
Compound plans advertise the larger completion context. Ordinary plans retain
the original 80-byte write boundary, so the new compiler also accepts old cores
without writing beyond their context; old compilers reject compound plans.

## Investigation

A separate CPU profile of updated main identified repeated trace preparation,
dispatch, completion and native instruction execution. A forward-successor-only
screen reduced progressing active time from 3,447.9 to 3,198.1 ms (7.25%). Adding
the BIOS transaction reduced a separate screen from 3,545.3 to 2,228.9 ms (37.13%);
all CPU/time, RAM, frame and PCM checkpoints matched. Diagnostic counters and
profiles remain external and are absent from the selected core.

Standalone return prediction plus loops with an entry prefix reduced direct
completions by about 4%, but a screen changed 2,249.9 to 2,243.6 ms (0.28%) while
growing generated code from 5,556,251 to 6,169,045 bytes. That prototype was removed.
These screens select a configuration; the repeated held-out controls above assess
its throughput and resource costs.

## Measurement scope

The frozen main is 6bb67bd7ed89d8697371715d02e255d9984e2e8b. Its CPU core is the
round 5 core; its complete runtime includes the newly imported presenter, HTML
and CSS. The normal-app driver freezes those assets, forces Original pixels,
and validates that the selected runtime actually served every required module.
All private media, guest state/code, profiles, PCM and captures stay outside Git.

Stationary controls include CPU work and software rasterization of the same
anchored scene. Fresh sequential main, selected and portable NP2kai runs rotate
order over three repetitions. Progressing controls alternate both cores per
slice and compare CPU/time, all conventional RAM, frame and PCM at setup and
8/16/24 guest seconds. Active time excludes asynchronous compile waits and
checkpoint hashing; stationary cold setup includes them. These are separate
measurements from real-time production audio playback.

The reference is portable Wasm NP2kai revision
5939e0c6d5985c4c08fc70f289a83290e5d3e6f7, Emscripten 6.0.13-git,
i486DX 65,894,400 Hz, 4 MiB, 86 sound and 48 kHz. Our Ap DX2 remains at 66 MHz
with the existing timing model. The parity criterion is median stationary host
time per guest second no greater than 1.05 times this reference, with at least
three fresh runs. It concerns this workload and these models; it does not
establish equal retired-instruction efficiency, universal game speed or playback
equivalence.
NP2kai cold setup is not instrumented by the comparison harness.

## Correctness and acceptance

Local validation passes 651 native tests with debug assertions and overflow checks,
all-target/all-feature Clippy and formatting, 37 optimized release-Wasm tests,
56 Node tests, 56 legal fixture/preparation checks plus 13 offline fetch tests,
and the publication audit of 31 assets. The public build and measured release
core have identical bytes.

Actual Chrome passes 20 JIT/fallback and CRT/fullscreen tests at both serving roots,
with the two existing intentional Pages skips for failure paths covered once at
the local root. Separate serial ADV98 playback passes both serving roots at 48 kHz
and 44.1 kHz, including Original pixels and sharp/soft/strong CRT windows. These
checks preserve existing assertions and timeouts. An unprofiled sound-enabled
private-game smoke uses the production timer and worker rasterizer: 23.324 wall
seconds advance 23.320 guest seconds, producing and consuming 1,119,374 samples
with zero underflows, overflows, drops, sequence/checksum errors or emulator
errors. It performs 105 HDD reads. This checks actual runtime adoption alongside
the fixed-work controls. Complete enabled application acceptance and its CI
revision are recorded in the PR. Required post-merge
verification is None; broader #172/#146 remain separate.

New native tests check branch promotion thresholds, target changes and reset,
and conservative compound admission with unusual CS/SS caches, CPL, IDT limits,
firmware requests and AH selection. Release-Wasm regressions check literal frame
words and 117-clock aligned transactions, a relocated odd IVT's 127-clock total,
work/event prefixes, live vectors/ROM/AH/groups, stack wrap/MMIO/self-code exits,
IVT/bitmap aliases, a preceding vector store, AC clearing, a later branch outcome
change and the legacy context canary. Existing interpreter comparisons also check
PCM bits, conventional RAM, pixels and CPU/timing metrics.

During test development, a reused core near its next audio event correctly
returned a one-instruction prefix when a literal test expected all four units.
That invalid-group test now uses fresh cores with room for the entire transaction;
the separate deadline tests retain partial-prefix expectations. The corrected
37-test release-Wasm suite passes.
