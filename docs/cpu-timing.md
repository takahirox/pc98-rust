# CPU clocks and guest time

Issue #140 replaces the unconditional eight scheduler clocks per instruction.
This is an instruction-table timing model with explicit bus policies, **not a
cycle-accurate simulation of either complete machine**. In particular, it does
not establish the cause of slow transitions in a particular game.

## Targets and references

| Profile | Selected CPU | Core clock | External bus | Device scheduler |
| --- | --- | --- | --- | --- |
| PC-9801VM | NEC µPD70116 V30, normal fast setting | 10 MHz | 16 bits, 10 MHz | 8 MHz |
| PC-9821Ap DOS | Intel486DX2-66 | 66 MHz | 32 bits, 33 MHz; legacy video/ports narrower | 8 MHz |

NEC's [VM4 specification](https://support.nec-lavie.jp/support/product/data/spec/cpu/b183-1.html)
lists switchable 8/10 MHz; this profile selects 10 MHz and does not yet expose the
8 MHz switch. The [Ap/U2 specification](https://support.nec-lavie.jp/support/product/data/spec/cpu/b211-1.html)
identifies the DX2-66. Frequencies are nominal integer rates; oscillator tolerance
and machine speed-switch circuitry are not modeled.

The current device configuration retains the synthetic BIOS's advertised
8 MHz-family timer setting: PIT input is 1,996,800 Hz on both profiles.
This is an explicit compatibility assumption, separate from CPU/core and bus
rates. NEC's [1993 hardware data book](https://vtda.org/docs/computing/NEC/PC-9800TechnicalDataBookHARDWARE%2BOCR_1993.pdf)
also documents a 2,457,600 Hz timer clock for the 5/10 MHz system family.
Board oscillator selection/coupling and the physical fast-VM/Ap default timer
configuration are not reproduced here. Cadence tests validate the configured
1,996,800 Hz domain; they do not establish the native oscillator configuration
of every machine bearing these model names.

Timing references, consulted as specifications rather than emulator code:

- NEC, [V20/V30 User's Manual, October 1986](https://www.dosdays.co.uk/media/nec/NEC-V20-V30-Users-Manual.pdf),
  section 12, especially pp.12-1, 12-4–20 (MOV/string), 12-30–60
  (arithmetic), 12-81–98 (bit operations), 12-100–141 (shifts), and
  12-142–176 (control/stack/prefixes). These include decoding, effective address,
  operand fetch and execution, assuming the instruction bytes are prefetched.
  Odd word operands add four clocks per additional 16-bit bus transfer; an
  odd read/modify/write adds eight. V30MZ and 8086 tables are not substitutes.
- Intel, [Intel486 DX2 Microprocessor Data Book, July 1992](https://bitsavers.trailing-edge.com/components/intel/80486/Intel486_DX2_Microprocessor_Data_Book_Jul92.pdf),
  section 10, pp.10-1–14, tables 10.1/10.2, and the interrupt table on p.10-13.
  These are **core** clocks. For example, DX2 IN/OUT cost 17/19 in real mode,
  CLTS costs 7, POP's short encoding costs 1, and LOOP costs 7/6 taken/not
  taken. Earlier 486DX data sheets/1990 PRM tables differ in these cases.
- Intel, [Intel486 Family Programmer's Reference Manual, 1992](https://bitsavers.trailing-edge.com/components/intel/80486/1992_Intel_486_Programmers_Reference_Manual.pdf),
  chapters 5, 9, 12 and 26 for cache/page controls, exceptions and restart.

## Accounting contract

`Cpu::step_timed[_with_observer]` returns `(StepTiming, Result<(), String>)`.
The timing survives failed execution and architectural rollback. Existing
`Cpu::step` and observer APIs remain convenience wrappers. CPU calls do not tick
devices, except existing synthetic firmware services that explicitly wait for
controllers. `StepTiming` separates execution, additional bus costs, idle core
clocks and already-applied firmware scheduler clocks. `cpu.cycles` counts core
execution/bus/idle clocks; synthetic firmware waits are reported separately and
are not invented guest CPU instructions.

`Machine` converts core clocks by `cycles * 8_000_000 / cpu_hz`, retaining the
integer remainder across every call. It then advances the existing `Bus::tick`
once. Firmware's already-applied device waits are not converted or ticked a
second time. Reset clears counters, cache timing state and the conversion
remainder. PIT, RTC, GDC scan/vertical sync, FDC/IDE, sound, PCM and mouse continue
to use their existing device clock inputs; no peripheral frequency changes with
CPU throughput. Sub-scheduler-clock precision is retained by the converter;
external events are observed at execution-unit boundaries, not within an opcode.

`cpu.retired` remains the historical execution-unit counter for compatibility.
`completed_instructions` counts completed architectural instructions (one REP
instruction, regardless of its length); `rep_elements` counts successful string
elements. Idle units retire neither. Faulting/retried instructions do not count as
completed instructions. The old fixed eight-clock instruction fallback is removed.
Unsupported instructions still stop explicitly; there is no generic cost for
successfully executed instructions missing a timing-table entry.

## Exact table cases and conditional costs

`src/timing.rs` covers every supported decoder class, including register/memory
and immediate forms, stack and near/far control transfers, conditional branches,
shifts, unsigned division, prefixes, V30 bit/BCD operations and supported i486
system/x87 metadata operations. Table values describe execution under the
reference assumptions, not measured total machine performance. DX2 address
calculation charges an additional core clock for an index and for a base changed
by the preceding instruction, and a displacement used with an immediate. The
optional index/immediate penalties are conservative policies; dependence tracking
observes changed register values and does not simulate pipeline forwarding.
NEC effective-address costs are already in its tables and are not charged again.

DX2 segment loads add 11 core clocks per previously unaccessed descriptor
(table 10.1, note 9), separately from the physical write that sets its Accessed
bit. This applies to data loads, far control transfers and interrupt entry;
VM86-to-monitor entry can load both an unaccessed code and stack descriptor.
Reloading an already accessed descriptor incurs no additional core clocks.

REP still executes at most one element per unit. Setup is charged on the first
unit, then only iteration cost on uninterrupted continuations. Zero-count REP
charges setup without an operand access. Conditional repeats stop on the actual
comparison result. An interrupt clears continuation state, so return to the
prefix pays setup again. Prefixes and fetch penalties are not repeatedly charged
on an uninterrupted REP. Existing STI/segment-load interrupt shadows remain at
architectural execution boundaries.

V30 MUL/IMUL/IDIV and CWD use the documented upper end of data-dependent ranges;
ENTER level 1 uses the neighboring documented setup policy. These are explicit
approximations. DX2 multiply uses the documented width-dependent upper bound.
DX2 bit scans use the documented operand-dependent formulas; carry rotates use
the count/operand-length formula. CMPXCHG distinguishes equal and unequal memory paths. Other paths without a
full microarchitectural model are marked approximate. No claim of exact data-dependent
multiply latency or calibrated game speed is made.

V30 POLL/WAIT (9Bh) uses NEC p.12-171's `2 + 5n` formula with one pin
sample (`n = 1`, seven core clocks), assuming the POLL pin is ready at the
first sample. It remains marked approximate: external POLL pin state and
additional sampling while a coprocessor is busy are not modeled.

V30 divide errors use NEC table 6-1's complete exception totals: 45 core
clocks for unsigned DIVU, and the 55-clock upper bound of signed DIV's
45–55 range. They replace successful division timing and include exception
entry; the external INT cost is not added. Actual operand/vector/stack bus
penalties and prefixes remain separate. Signed data dependence and fault
detection details remain approximate.

## Physical bus assumptions and remaining limits

| Effect | Implemented policy | Accuracy boundary |
| --- | --- | --- |
| V30 ordinary RAM | Aligned nominal transfers included in NEC tables; odd word +4 per access | Prefetch starvation, refresh and DMA HOLD contention absent |
| V30 text/graphics VRAM | Two additional waits per 16-bit beat | Testable board assumption; no measured scan arbitration |
| DX2 RAM/cache | Timing-only unified 8 KiB, four ways, 16-byte lines, 128 sets; round-robin replacement | Values always come from Bus; no stale cached data |
| DX2 read miss | Serialized 2-1-1-1 external line fill, 10 core clocks | Conservative completion cost; no critical-word overlap/prefetch pipeline |
| DX2 cache disabled | Four core clocks per aligned 32-bit read/write beat | Nominal 2-clock external bus, no DRAM row/refresh model |
| DX2 writes | Write-through, no write allocate, serialize four core clocks | Write-buffer overlap/fullness not simulated |
| DX2 legacy VRAM | Uncached, 16-bit beats, eight core clocks per beat | Two external transfer clocks plus two wait clocks; no EGC/PEGC contention model |
| DX2 misalignment | Three additional core clocks per unaligned operand | Plus actual split physical beats |
| Port I/O | CPU table cost; V30 odd word +4; DX2 extra bytes +4 core clocks each | Devices expose byte handshakes, no board-ready waveform |
| Page walks/system descriptors | Physical memory accesses contribute bus cost | TLB replacement remains existing policy; A/D update and protection microcode are approximate |
| Interrupts/faults | V30 divide uses complete exception totals; other entry charged in addition to interrupted/faulting work; each protected retry charged; software INT has its own table entry | Fault detection point and double-fault pipeline costs are conservative, not measured |
| HLT/shutdown | One microsecond idle quantum; clocks/events continue, retirement stops | IRQ wake-up quantized to one microsecond |
| Synthetic firmware | Existing explicit FDC waits plus actual guest thunk instructions; flush timing tags after host services | BIOS/LIO host work has no fictitious instruction count; ROM execution latency is not reproduced |

Cache residency respects CR0.CD; reset starts cache-disabled. Writes from outside
CPU execution invalidate matching timing tags. Cache replacement, fill serialization,
ROM caching, memory-region wait policies, page cacheability controls, DMA snooping
during host firmware, and the absence of a V30 prefetch queue are limitations of
this approximation. Table-derived core times and approximate board costs are
separate fields. `approximated_units` exposes uses of policy/range/fault/firmware
paths; it is not a certification that all other hardware effects are exact.

## Browser pacing and diagnostics

The worker requests 80,000 **device** clocks (10 ms) at a time through
`step_scheduler`, with a separate 100,000-unit work cap. It retains a partial
slice's guest deadline and yields before continuing. Audio still targets a bounded
60 ms reserve at 48 kHz. Muted pacing uses wall-clock deadlines with bounded debt;
pause/reset/flush drop the deadline. The CPU instruction count no longer decides
how much audio or guest time a slice produces.

Host execution avoids copying an empty TLB into every instruction rollback
snapshot. Populated TLB snapshots share immutable storage and copy on mutation,
retaining independent fault/retry state. Register-dependence tracking compares
the low and upper words directly; segmentation and paging fast paths retain all
bounds, access and permission checks. These reduce host interpreter overhead;
they do not alter instruction clocks, bus policies, audio reserves or work caps.

Wasm metrics retain existing IDs: 0 is execution units, 5 device clocks. New IDs
26–31 expose core clocks, completed instructions, REP elements, guest seconds,
approximated units and core frequency. Worker statistics add these counters and
host instructions/second. Diagnostic `transitionStart()`/`transitionDuration()`
measure guest seconds and host milliseconds separately. Diagnostic `advance(us)`
is an explicit device-time wait for integration tests; `core.step(n)` continues
to mean bounded CPU units, including one-unit tracing.

## Validation and hardware comparison

`tests/cpu_timing.rs` uses literal NEC and DX2 table expectations, distinguishes
execution clocks from physical penalties, and checks REP setup/zero/early exit,
interrupt restart/shadows, faults, fractional conversion and halted device time.
Existing peripheral tests retain their independent literal clock traces.
Timing-sensitive integration probes use channel-2 PIT waits or vertical periods
rather than assuming an instruction takes eight scheduler clocks. FDC byte ISRs
sample PIC state on result interrupts so their 500-kbit/s service loop fits the
V30 execution budget; payloads, IRQ counts and disk equality remain asserted.

No target-hardware traces, licensed game timings, physical BGM recordings, or
measured DRAM/VRAM wait-state data were supplied. Analytical instruction-table
comparisons and legal guest regressions can validate the implemented contract;
they cannot validate board arbitration, microarchitectural overlap, or establish
a game's screen-transition root cause. Those effects remain explicitly unmodeled.

The [validation record](cpu-timing-validation.md) lists executed checks, workload
measurements, fixture changes and unverified hardware comparisons.
