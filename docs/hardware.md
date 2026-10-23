# Machine profile and implementation boundaries

This document describes the preserved VM default. The new selectable
[Ap DOS subset, independent guests and exact limits](dos-compatibility.md) are
documented separately; it does not implement a complete PC-9821.
The [Issue #11 device audit](ap-devices.md) describes the Ap-only IDE, memory,
EGC/PEGC and sound additions and their guest fixtures. The port table below
otherwise describes the preserved VM behavior; both profiles expose the PC-98
bus mouse's 7FD9/7FDD/7FDF/BFDB interface.

The default target is a PC-9801VM-family V30 in 16-bit real mode, with 640 KiB
conventional RAM, 12 KiB text storage, 192 KiB planar graphics storage and two floppy slots.
The scheduling clock is fixed at 8 MHz. The
[NEC VM4 specification](https://support.nec-lavie.jp/support/product/data/spec/cpu/b183-1.html)
and [generation overview](https://pc98.ne.jp/techdet/) identify the model boundaries.
This VM profile has no 286, 386, PC-9821 or IBM VGA compatibility mode.

All hardware implementations are original Rust. `Cargo.lock` contains only this
package; `cargo tree` has no emulator, Wasm binding, or other Rust dependencies.
Assembly is original except the explicitly attributed hello-world fixture. No
MAME/NP2/DOSBox code, translated implementation, ROM or emulator core was used.
FreeDOS source is consulted only for its guest BIOS call contracts, and executes
as guest code. Hardware references are specifications and independent technical
documentation, not copied implementations.

## Address map

The [VM map diagram](https://commons.wikimedia.org/wiki/File:PC-9801VM_memory_map.svg)
and [UNDOCUMENTED memory/system data](https://www.webtech.co.jp/company/doc/undocumented_mem/memsys.txt)
provide the map and BIOS work-area references. This is the configured subset;
model windows without an installed device read `FF` and ignore writes.

| Physical range | Implemented mapping |
| --- | --- |
| `00000–003FF` | Writable interrupt vector table |
| `00400–005FF` | BIOS work area in conventional RAM |
| `00600–9FFFF` | Conventional RAM (640 KiB including the two rows above) |
| `A0000–A1FFF` | Text characters (low bytes of words; 2048 cells) |
| `A2000–A2FFF` | Text attributes (low bytes of words) |
| `A3000–A3FFF` | Character-generator window uninstalled; no Kanji ROM |
| `A4000–A7FFF` | Uninstalled windows, open bus |
| `A8000–AFFFF` | Blue plane, selected CPU page, 32 KiB |
| `B0000–B7FFF` | Red plane, selected CPU page, 32 KiB |
| `B8000–BFFFF` | Green plane, selected CPU page, 32 KiB |
| `C0000–EFFFF` | Expansion/BASIC/other model ROM windows uninstalled |
| `F0000–FFFFF` | Write-protected original synthetic firmware, including reset jump at `FFFF0` |

Each graphics page has three 32 KiB planes; two pages total 192 KiB. Display and
CPU-access pages are independent. Pixels use 80 bytes per row, MSB first, 400 rows.
This deliberately differs from IBM VGA memory and registers. Real-mode physical
addresses wrap at 20 bits; segment offsets and register/stack operations wrap at
16 bits. Tests include both boundaries and ROM write protection.

## CPU and scheduler

For the default VM profile, `src/cpu.rs` implements 8/16-bit registers, segment
overrides, all 8086 ModR/M effective-address combinations, ALU and flag logic, decimal adjustment,
multiply/divide, shifts/rotates, stack, near/far call/jump/return, conditionals,
string operations and REP/REPE/REPNE, interrupt entry/IRET, byte/word IN/OUT,
CLI/STI/HLT. Immediate push, PUSHA/POPA, ENTER/LEAVE and immediate shifts implement
the 186-compatible forms available on V30. TEST1/CLR1/SET1/NOT1 implement V30
single-bit operations. AAM/AAD consume but ignore the encoded radix, using decimal
10 as on V30. See the [NEC V20/V30 user manual](https://www.dosdays.co.uk/media/nec/NEC-V20-V30-Users-Manual.pdf)
and [NEC V-Series data book](https://bitsavers.org/components/nec/_dataBooks/1990_NEC_16-bit_V-Series_Microprocessor_Data_Book.pdf).

Unimplemented opcodes fail with CS:IP and opcode information. V30 8080 emulation
mode, packed BCD string extensions, bit-field extensions, coprocessor/ESC, BOUND,
TF single-step traps, NMI, undocumented opcodes and later x86 extensions are not
implemented in VM. LOCK has no bus-contention effect in this single-CPU machine.

The selectable Ap profile extends this same decoder with 32-bit operands and
addresses, SIB, FS/GS and selected 386/486 real-mode integer instructions. It checks
64 KiB segment limits and delivers guest exceptions; it does not provide protected
mode, DPMI or an FPU. See the [exact Ap CPU coverage and independent DOS diagnostic](dos-compatibility.md#issue-9-cpu-coverage-and-independent-guest-evidence).
VM retains the V30 behavior described above.
Ap also provides the [real-mode GDTR](#ap-real-mode-gdtr) and
[IDTR prerequisites](#ap-real-mode-idtr), plus
[SMSW status readback](#ap-real-mode-smsw) below.

An instruction or one REP element consumes one 8-clock scheduling slot. This is a
deterministic work/cycle model, **not instruction-cycle accuracy**. Firmware FDC
transactions additionally advance the same device scheduler. HLT stops instruction
retirement while clocks and IRQs continue. STI and MOV/POP SS inhibit delivery for
the next instruction. A frame runs at most two slices of 16,667 slots; the core
caps each call at 100,000 slots. There is no unbounded catch-up after a suspended
tab. This bounds UI work even for infinite loops and maximum-length REP strings.

## Ap real-mode SMSW

Ap implements `0F 01 /4` SMSW from one CPU-local machine-status word representing
CR0 bits 15:0. Construction and Machine reset expose **0010h**: ET set;
PE, MP, EM, TS and NE clear. Reserved low-word bits are zero by explicit emulator
policy. This follows [Intel486 PRM §10.1, Figure 10-2](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143).
ET readback does not establish working x87 support.

Register destinations replace only the low word. Operand16 preserves the upper
half; operand32 also preserves it as a deterministic emulator policy where Intel
specifies undefined upper bits. Memory destinations receive exactly two
little-endian bytes with either operand size. Flags and unrelated architectural
state are preserved. Address size independently selects effective-address decoding,
including ModR/M, SIB, default SS and segment overrides. Repeated size overrides
select the override once; the final segment override wins. See the
[Intel SDM SMSW entry, printed pp. 4-647–648](https://cdrdv2-public.intel.com/782151/253667-sdm-vol-2b.pdf#page=649).

Both memory bytes are preflighted before writing through the existing Bus, so A20
mapping and ROM write protection apply. **FFFEh** is the last valid start;
**FFFFh** and address32 offsets above it raise restartable exception **13**, including
SS-selected operands, following the **Intel486 PRM SMSW real-address table,
printed p. 26-268**. The modern SDM instead lists exception **12** for SS overruns;
these tables disagree, and this instruction deliberately uses the target-era
contract. General memory helpers retain their existing SS exception behavior.
LOCK on either register or memory forms raises exception **6**. Faults save the
first prefix IP in the existing six-byte real-mode frame and permit guest
repair/IRET/retry. VM retains its unsupported V30 `0F 01` decoder behavior.

The original MIT [boot probe](../guest/smsw.asm) builds offline with
`python3 tools/prepare_smsw_fixture.py`. Its
[manifest](../fixtures/smsw-manifest.json) pins source/boot/image hashes, assembler
command, exact results/guards, license and two-million-slot budget. Native and
production release Wasm tests boot the identical image before and after reset,
with browser coverage at `/` and `/pc98-rust/`. They require `SMSW PASS`, no `FAIL`
or host error, exact results and unchanged disk bytes. Generated fixtures and
license/source copies stay outside publication assets.

This adds readback only. Guest status writers, MOV control-register instructions,
LMSW, CLTS, protected execution, paging and x87 execution remain unimplemented.
No whole inventory category is promoted to verified by this bounded acceptance.

## Ap real-mode GDTR

Ap implements `0F 01 /2` LGDT and `0F 01 /0` SGDT with memory operands.
LGDT reads a little-endian 16-bit limit and a 24-bit base by default (clearing
bits 31–24), or a 32-bit base with `66h`. SGDT always writes exactly six bytes:
the limit and **full 32-bit base**, with either operand size. Operand and address
sizes are independent; existing ModR/M/SIB and segment overrides apply. Flags,
general registers and segment registers are preserved on success.

GDTR belongs to each CPU instance. Construction and Machine reset choose
limit **0**, base **0** for deterministic emulation. This is emulator policy,
not a hardware reset guarantee: Intel486 PRM §10.1 leaves power-up GDTR undefined.
The base is linear metadata, independent of RAM capacity and A20; LGDT never
accesses the referenced table. Operand accesses still use the Bus, including
A20 mapping and firmware write protection.

Both instructions preflight the complete six-byte operand. `FFFAh` is the last
legal starting offset; `FFFBh` or any address32 offset beyond `FFFFh` raises
restartable exception 13, **including SS-selected operands**. Register operands
and LOCK raise exception 6. Fault frames save the first prefix IP through the
existing real-mode IVT mechanism, before any GDTR change or destination write.

This follows the [Intel486 Programmer’s Reference Manual](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143),
pp. 26-193–194 and 26-261. Its LGDT discussion explicitly specifies full-base
SGDT stores, while its older SGDT description/compatibility notes conflict.
We choose the explicit full-base store operation in the
[Intel SDM SGDT entry](https://cdrdv2-public.intel.com/835757/325383-sdm-vol-2abcd.pdf),
not the older truncated-base interpretation. The real-address exception contract
also agrees with the [SDM SGDT exception table](https://cdrdv2-public.intel.com/782151/253667-sdm-vol-2b.pdf).

`0F 01 /4` through `/7` remain explicit unsupported errors. VM keeps its V30
decoder. LIDT/SIDT are documented below. These prerequisites add no CR0/PE
transitions, descriptor
lookup/validation or caches, privilege changes, paging, task switching, FPU or
Windows support. Protected mode remains unimplemented; CPU386 and integration
remain partial.

The original MIT [boot probe](../guest/gdtr.asm) builds offline with
`python3 tools/prepare_gdtr_fixture.py`. Its [manifest](../fixtures/gdtr-manifest.json)
records assembly, license, hashes and the two-million-slot budget. Native
`cargo test --locked --test cpu386 gdtr_` and release-Wasm browser tests require
`GDTR PASS`, no `FAIL` or host error, exact result/guard bytes, and an unchanged
disk on Ap before and after reset. Fixtures stay outside `web/`.

## Ap real-mode IDTR

Ap implements memory `0F 01 /3` LIDT and `0F 01 /1` SIDT. Each CPU resets
IDTR to base **0**, inclusive byte limit **03FFh**. LIDT reads a little-endian
word limit and 24 base bits with operand16 (clearing the high byte), or all
32 base bits with operand32. Address size is independent. SIDT writes exactly
six bytes, including the full base with either operand size, preserving flags
and registers. LIDT retains linear metadata without accessing the table.
Operands and vector bytes use existing Bus/A20 mapping and ROM protection.

Both instructions preflight all six operand bytes: `FFFAh` succeeds, while
`FFFBh` and address32 offsets above `FFFFh` raise restartable exception 13,
including SS-selected operands. Register forms and LOCK raise exception 6.
These instruction-specific real-address rules follow Intel486 PRM Table 10-1
and pp. 26-193–194, 26-261; they intentionally differ from ordinary SS memory
accesses. The [modern SIDT entry](https://cdrdv2-public.intel.com/782151/253667-sdm-vol-2b.pdf#page=645),
printed pp. 4-643–644, lists exception 12 for SS overruns. Its explicit operation
stores the full 32-bit base for both operand sizes; that operation is selected
over its contradictory compatibility paragraph claiming a zero upper byte.
The older PRM description calls the sixth byte undefined, and its compatibility
note says zero for operand16. Tests require high-byte retention rather than
those older interpretations.

All Ap interrupt sources use `IDTR.base + 4 * vector`, with wrapping 32-bit
linear arithmetic before Bus mapping. The full four-byte entry must fit the
inclusive limit; entries remain word offset/word segment pointers. A table-limit
failure raises exception 8 before writing any original frame. Software INT faults
save the first-prefix IP, as specified by the
[i486 datasheet §3.5, Table 3.1](https://device.report/m/d0d81f608917b4fb1baa0da42c97dede73f88575259f19de2d24492a995d89ff),
printed pp. 43–44. Exceptions still push no error code in real mode; existing
six-byte frames, resume/restart distinctions, IF/TF/AC clearing, interrupt shadows
and IRET behavior are preserved. See also
[Intel486 PRM §22.3](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143).

If vector 8 itself cannot fit (`0022h` versus the sufficient `0023h`), delivery
enters bounded CPU shutdown. This state stops instruction execution and maskable
IRQ acknowledgement, remains distinct from HLT, and clears on Machine reset.
Native `Cpu.shutdown` and Wasm inspection metric 16 expose it to host assertions.
No NMI recovery, general nested-fault handling or invalid-stack overhaul is added.
VM retains its V30 decoder and fixed address-zero IVT.

The original MIT [IDTR boot probe](../guest/idtr.asm) builds offline with
`python3 tools/prepare_idtr_fixture.py`. The [manifest](../fixtures/idtr-manifest.json)
records source/license, assembly, hashes, exact result/guard bytes and a maximum
2,000,000-slot budget. Native and production release-Wasm tests at `/` and
`/pc98-rust/` execute the same image before and after reset, require explicit
PASS without FAIL, exact bytes, unchanged disk data and terminal shutdown.
Fixtures remain outside `web/`.

Protected mode and CPU386 remain partial. CR0/PE transitions, protected gates,
descriptor lookup/caches, privilege transitions, LDTR/TR, task switching, paging,
x87, NMI, DPMI, and Windows compatibility remain outstanding. These probes do
not establish full protected-mode accuracy or commercial software compatibility.

## Device ports and interrupts

Port facts are traced to the individual chapters of
[UNDOCUMENTED 9801/9821 Vol.2](https://www.webtech.co.jp/company/doc/undocumented_mem/index.html).
Registers below implement the behavior needed by the three accepted workflows;
this table is not a claim of complete chip emulation.

| Ports (hex) | Device and supported behavior | IRQ / notes |
| --- | --- | --- |
| `00,02`; `08,0A` | Master/slave 8259: existing ICW path, masks, IRR/ISR, OCW2 programmable/rotating priority, specific/nonspecific EOI, auto EOI, automatic rotation and OCW3 special-mask mode | Master IR7 cascades slave; each controller resolves its own priority and ISR blocking |
| `01–0F` odd, `11–1F` odd, `21,23,25,27` | 8237 address/count flip-flop, controller disable, channel masks, direction/decrement, terminal count and auto-init reload, master clear, bank registers | FDC uses channel 2; count/address change with actual byte transfers |
| `31,33,35,37` | PPI startup inputs, output C, bit set/reset, active-low beep gate | Input straps are fixed VM-profile constants |
| `41,43` | Keyboard byte FIFO, ready status, command acknowledgements, down/up scan bytes | IR1 / INT 09; firmware maintains BIOS ring, held-key bitmap and modifier state |
| `60,62` | Master text GDC parameter/status and command/data readback | RESET, display enable, PITCH, CURS, CCHAR, PRAM start, CSRR |
| `A0,A2` | Slave bitmap GDC command/parameter/status/readback | Same register subset; 640×400 direct VRAM path |
| `68` | Text display enable; graphics 200/400-line selection | Fixed 80-column 25-row text; 200-line graphics doubles rows |
| `6A,A8,AA,AC,AE` | Digital 8-color pairs; analog index and 4-bit RGB levels when enabled | Eight palette entries only; no fourth graphics plane / 16-color board |
| `A4,A6` | Display and CPU graphics page select | Two independent three-plane pages |
| `7C,7E` | GRCG mode/tile writes | TDW and RMW, plane masks; no EGC |
| `71,73,75,77`; `3FD9–3FDF` odd | 8253 channels, control/access modes, low/high writes, counter latches | Channel 0 IR0 / INT 08; channel 1 beep; VM input clock 1,996,800 Hz |
| `90,92,94` | 2HD FDC status/FIFO/control, SPECIFY, sense status/interrupt, recalibrate, seek, read ID, READ/WRITE DATA through DMA, motor/DMA-enable gating | IR11 / INT 13; delayed command completion and seven-byte transfer results |
| `BE` | Fixed 1 MB floppy interface selection | No alternate 640 KB controller at C8/CA |
| `40,5C,5E,64,6E` | Fixed system/display inputs and deterministic clock status | See `Bus::input`; no RTC serial protocol |
| `F0` | Private synthetic-firmware thunk ABI, accepted only from its ROM stubs | Not a guest DOS API or claim of a real VM hardware port |

### 8259A OCW2 priority subset

Both profiles implement independent priority order and automatic-rotation state
for each PIC at the ports above. Lowest-priority line L gives highest-first order
`(L+1) mod 8` through L. Arbitration chooses the first unmasked IRR bit above the
highest-priority active ISR bit. Masking an in-service line does not remove its
normal-mode blocking. Nonspecific EOI clears the highest-priority ISR bit under
the current order; specific EOI clears the named bit.

| OCW2 (hex; n = bits 2:0) | Effect |
| --- | --- |
| `00` | Disable rotation on automatic EOI, preserving the priority order |
| `20` | Nonspecific EOI without rotation |
| `40` | No operation |
| `60+n` | Specific EOI without rotation |
| `80` | Enable rotation on automatic EOI |
| `A0` | Nonspecific EOI; completed line becomes lowest |
| `C0+n` | Set line n lowest, without clearing ISR |
| `E0+n` | Specific EOI; line n becomes lowest |

With ICW4 AEOI enabled, successful acknowledgement clears the request without
retaining ISR, and makes the acknowledged line lowest if automatic rotation is
enabled. Requests, `pending()`, status reads and failed acknowledgements do not
rotate. With AEOI disabled, the automatic-rotation enable has no effect on
acknowledgement or ordinary EOI. Empty-ISR nonspecific EOI, including rotating EOI,
does nothing. Specific rotation always sets the explicitly named line lowest.
IRR and IMR are preserved by priority and EOI commands.

Construction and Machine reset deterministically start with IR0 highest, IR7
lowest and automatic rotation disabled. This is the emulator's initialization
policy, rather than a model of uninitialized chip power-on state. ICW1 restores
that priority/rotation state, clears AEOI pending ICW4, and selects IRR readback.
The existing ICW sequencing and fixed master-IR7 cascade abstraction are retained.
Slave delivery sets ISR independently on both controllers in normal EOI mode;
each requires its own EOI, and pending requests survive deferral.

The specification is Intel's [8259A datasheet, initialization and Figure 8,
printed pp. 9–16](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=9).
Original [native coverage](../tests/pic.rs) checks all eight lowest-priority
settings, all OCW2 families, masks, reordered nested ISR, AEOI, reset and cascade
ports. The [MIT boot probe](../guest/pic.asm) uses only hardware PIT/keyboard IRQs,
confirming both IRR bits before each STI; native and release-Wasm hosts compare
its exact trace, readback, guards and disk bytes before and after reset. The
[manifest](../fixtures/pic-manifest.json) pins source/image hashes, assembler
command, result bytes and the 2,000,000-slot budget. See the
[validation record](validation.md#issue-41-8259a-ocw2-priority-2026-10-23).

### 8259A OCW3 special-mask subset

Each controller independently latches OCW3 SMM (bit 5) only when ESMM (bit 6)
is set: `68h` enables special-mask mode and `48h` disables it. With ESMM clear,
the mode is preserved. RR/RIS status selection operates independently, including
combined commands `6Ah`, `6Bh`, `4Ah` and `4Bh`; repeated reads are passive.

In special-mask mode, masked ISR bits do not block arbitration. Unmasked ISR bits
still block equal/lower priorities under the programmed order. IMR continues to
inhibit requests without clearing IRR. Nonspecific EOI selects the highest-priority
unmasked ISR bit; rotating nonspecific EOI uses the same eligible set and makes
the selected line lowest. With no eligible ISR bit, neither operation changes ISR
or priority. Specific EOI can clear a masked ISR bit, and specific rotation keeps
its existing explicit-line behavior. Mode changes preserve IRR, ISR, IMR, priority
and AEOI/automatic-rotation state. ICW1 clears special-mask mode; construction and
Machine reset also disable it as deterministic emulator policy.

The reference is [Intel 8259A, Figure 8, OCW3/EOI and special-mask operation,
printed pp. 13–16](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=13).
[Native coverage](../tests/pic.rs) exercises all eight priority origins, mode/mask
transitions, EOI selection, initialization, reset, instance isolation and both
controllers' ports, including explicit master EOIs for slave delivery. The
separate original MIT [nesting probe](../guest/pic_smm.asm) uses physical keyboard
input and a guest-programmed one-shot timer. It confirms both pending requests,
observes normal-mode blocking with IF set, then enables special-mask nesting.
Native and production-Wasm tests require trace `01 00 01`, ISR snapshots
`02 03 02`, timer counts zero before enabling and one afterward, exact results
and guards, unchanged disk bytes and `PIC SMM PASS` within 2,000,000 slots per
boot. The nested timer uses nonspecific EOI; the outer keyboard handler finishes
with specific EOI. Both profiles run before and after reset; browser coverage
includes both URL roots. The [manifest](../fixtures/pic-smm-manifest.json) pins
offline preparation, source/image hashes and exact bytes. See the
[validation record](validation.md#issue-43-8259a-ocw3-special-mask-mode-2026-10-23).

PIC remains **partial**. Polling, special fully nested mode,
general ICW sequencing/configuration, alternate cascade wiring, edge/level
electrical modeling and spurious-interrupt fidelity remain outside this subset.
These bounded checks do not establish full hardware accuracy or commercial
software compatibility.

The hello source uses `OUT 3FDFh,AL`, an immediate-port encoding that truncates to
`DFh`. That timer mirror is decoded too. Full PIT gate/readback/mode waveforms,
DMA arbitration and request modes, precise FDC motor/rotation timing, FDC programmed
I/O transfers, formatter/scanner commands, GDC vector/area drawing and all GDC
raster partitions are outside the tested subset. Unsupported FDC commands return
an invalid-command result. No pretend FM register bank is implemented.

GDC status at `0060h` and `00A0h` now reports horizontal blank (bit 6) from
elapsed guest clocks. The supported fixed 640×400 modes approximate horizontal
display timing at the existing 8 MHz scheduler clock:

| Profile | Line period | Active interval | Horizontal blank interval |
| --- | --- | --- | --- |
| VM, approximately 24 kHz class | 320 clocks (40 µs, 25 kHz) | Phases 0–255 (32 µs) | Phases 256–319 (8 µs, 20%) |
| Ap, approximately 31 kHz class | 256 clocks (32 µs, 31.25 kHz) | Phases 0–199 (25 µs) | Phases 200–255 (7 µs, 21.875%) |

Phase is `bus.cycles % line_period`; it starts at zero (bit 6 clear) on machine
creation/reset and advances even without port reads or with display disabled.
Text and bitmap GDCs share this phase. Firmware mode initialization, BIOS display
commands, GDC RESET/START/STOP, and PITCH do not restart it. SYNC/RESET timing
parameters, porch subdivision, monitor-frequency switching, zoom and interlace
are not modeled. These fixed intervals are deliberate integer-clock approximations,
not exact physical VM/Ap frequencies or programmable uPD7220 timing. They apply to
both existing Ap planar and packed-pixel display paths; no new PEGC mode is added.

The [PC-98 display-port definitions](https://www.webtech.co.jp/company/doc/undocumented_mem/io_disp.txt)
identify bit 6 at both ports and the master/slave display synchronization. The
[NEC uPD7220 Design Manual, chapter 6](https://ftpmirror.your.org/pub/misc/bitsavers/components/nec/uPD72220_GDC_Design_Manual_Version_3.pdf)
describes the active interval and blanked front porch, sync and back porch that
motivate this approximation. The model uses no guest identity or port-read count.

Existing status semantics are preserved: bit 5 is set when
`bus.cycles % 133333 > 125000`, bit 2 stays set, and bit 0 reflects the selected
GDC's actual readback FIFO. Vertical status and IRQ timing retain their independent
approximation; horizontal and vertical phases are not a complete physical raster.
The renderer composites planar graphics with text attributes (visible, reverse, underline and
color), cursor and GDC display start. Text scrolling also works through guest RAM
copies, as used by FreeDOS. Glyphs are newly drawn 5×7 Latin shapes expanded into
8×16 cells; lowercase uses uppercase shapes and unknown glyphs use a question box.
No NEC font or Kanji data is present.

PIT clocks generate capped 48 kHz mono PCM in Rust using the beep gate and counter
phase, including pulses within a browser frame. The host copies those samples to
Web Audio buffers after a user gesture. FM/OPNA is absent; none of the acceptance
guests needs it. The output is a square wave approximation, not analog speaker
fidelity.

### DMA terminal count and auto-initialization

The four existing channels retain base and current 16-bit address/count registers.
Byte writes update the selected byte of each through the shared first/last
flip-flop; reads expose current values. Each serviced byte increments/decrements
the address without carrying into the existing PC-98 bank register and decrements
the count. Count zero transfers one byte; `FFFFh` transfers 65,536. Decrementing
zero generates a terminal-count event. Mode bit 4 reloads both current registers
from their bases and preserves the mask; without it the count becomes `FFFFh`
and the channel is masked. Either case latches TC status until status is read.
FDC READ/WRITE DATA consumes the event from that byte transfer, independently of
register reload and previously latched status, then produces seven result bytes
and the completion IRQ on both VM and Ap.

Writing any byte to software master clear (`001Bh`) between device services
clears the command register, TC status latch and shared first/last byte pointer and masks all four
channels, leaving the modeled controller idle. Current and base address/count
registers and mode bytes survive unchanged, including partial-byte programming;
master clear does not reload bases. Unmasking allows the existing FDC consumer to
resume with retained programming. Intel's selective reset list (printed pp. 2
and 9, Figure 6) excludes address/count/mode; their retention follows from that
list. NEC [IEA-751](https://www.renesas.com/ja/document/apn/upd72070-application-note),
§2.4, Table 2-10 (printed p. 15), maps master clear to `1Bh` and byte-pointer clear
to `19h`. Request and Temporary registers are unmodeled; their reset behavior
is not claimed. Machine construction/reset clears the command register. The existing
policy of zeroing external PC-98 bank registers on master clear is preserved;
board-level bank-reset wiring remains unverified. FDC retention tests use bank zero.
See [master-clear evidence](validation.md#issue-31-dma-master-clear-retention).

Writing command port `0011h` models only bit 2, the active-high controller
disable bit, on both VM and Ap. Setting it blocks DMA service independently of
channel masks; clearing it permits service only on an unmasked channel. Other
command bits are ignored and remain unsupported. Repeated command writes retain
current/base address/count programming, modes, banks, masks, TC status and the
shared byte pointer. Programming remains available while disabled. Reading
`0011h` returns and clears status, rather than reading back the command.

The atomic FDC backend samples eligibility at its scheduled service boundary,
including command writes after submission but before service. A valid data
command with DMA disabled follows the masked-channel no-service path: no payload
or DMA progress, abnormal ST0 and ST1 overrun bit 4, seven result bytes and a
completion IRQ. Clearing disable afterward allows a fresh command to use retained
programming; it does not replay the completed error. This is a bounded scheduling
approximation, with no claim about physical timeout duration, DRQ/DACK waveforms
or magnetic-media effects. Intel's register descriptions and Figure 6 (printed
pp. 7–11) specify command/reset semantics; NEC IEA-751 Table 2-10 maps command to
`11h`, and the [µPD765A/B Table 3](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/UPD765_Datasheet_OCRed.pdf)
(PDF p. 10) defines overrun. See [controller-disable evidence](validation.md#issue-33-dma-controller-disable).

This bounded contract follows Intel's [8237A datasheet](https://read.seas.harvard.edu/~kohler/class/04f-aos/ref/hardware/8237A.pdf),
printed pp. 5–10 (autoinitialize and register descriptions), and NEC's
[µPD765A datasheet](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/uPD765A.pdf)
(TC and processor interface). `tests/dma.rs` covers all four channels, both address
directions, byte reprogramming, wrap, repeated service and raw FDC commands;
the original MIT boot probe executes on native and production Wasm at both browser
serving paths. See [execution evidence](validation.md#issue-29-dma-terminal-count-auto-init).

DMA remains **partial**. External EOP injection, demand/block/single arbitration,
software requests, other command controls, verify transfers, memory-to-memory DMA,
cascade/rotating priority, bank-reset fidelity and additional device interactions
still require implementation or verification. Broader FDC commands and precise motor/rotation timing remain
outside this contract. Passing these checks establishes terminal-count reload,
software master clear, controller disable and their existing FDC consumer, not complete hardware
accuracy or commercial compatibility. Other hardware inventory requirements remain unchanged.

## Synthetic firmware and boot

`guest/firmware.asm` is assembled to the original IPL in `firmware.bin`. It runs
from the CPU reset vector, programs PIC/PIT/GDC, requests the first 1024-byte sector
through INT 1Bh, then far-jumps to `1FC0:0000`. Four-byte ROM service thunks execute
OUT/IRET; original Rust firmware routines implement BIOS services. There are no
ROM dumps and no host INT 21h implementation. Disk firmware uses the same FDC
command/result FIFO, seek, DMA and scheduler paths as guest-issued I/O.

| Entry | Supported services |
| --- | --- |
| INT 08–17 | Timer tick, keyboard scan processing and appropriate PIC EOI; guest-installed vectors supersede firmware |
| INT 18 AH=00,01,02,03,04,05 | Blocking get, peek, modifier status, FIFO clear, held-key group, conditional get |
| INT 18 AH=0A–0E,10–14,16–18 | CRT mode query/set, enable/disable, start, cursor, replacement font read, fill, beep on/off |
| INT 18 AH=40–42 | Bitmap display on/off, 640×400 color mode |
| INT 18 AH=4A (Ap only) | Flash/flashless drawing timing selection; see contract below |
| INT 1B low AH=0,3,4,7,A | Reset/recalibrate, sense and read ID; unavailable drive reports carry/error |
| INT 1B low AH=1,5,6 | Verify/read, write, read; CHRN and ES:BP/BX byte-count contract; uses controller/DMA |
| INT 1C AH=0,2 | Fixed deterministic BCD RTC query (2026-01-01) and one-shot interval callback (see below) |
| INT 1F | Unsupported extended memory, returns carry set |
| Guest-registered F990h ROM entry | Graph LIO GINIT; other directory entries fail explicitly (see below) |

Unknown BIOS services fail visibly except INT 1F's documented unavailable-memory
response and disk-controller error returns. Verify currently performs an actual
read into the supplied buffer. Bootstrap is 1 KiB-sector media; other raw geometries
are accepted for data, but boot firmware for 512-byte-sector IPLs is not validated.
No BASIC fallback, proprietary firmware identity string, HDD bootstrap or extended
memory is provided. Firmware work-area initialization includes conventional RAM
size, floppy equipment, boot DA/UA, text attributes and keyboard ring pointers.

### BIOS held-key bitmap

The work-area table at `0000:052A–0539` has 16 groups of eight scan codes.
For scan `s`, byte `052A + s/8`, bit `s%8` is one while held. Return (`1Ch`)
therefore uses `052Dh`, bit 4. This layout follows the
[PC-9801 Programmer's Bible, printed p.69](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf);
[NEC's BIOS databook, printed pp.32–33](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
describes INT 18h/AH=04h grouped sensing (AL selects the group, AH returns it).

Host input queues make/break scan bytes and requests IRQ1. The firmware checks
ready status at `43h`, consumes one byte from `41h`, sets/clears its bitmap bit,
processes the existing character ring/modifiers, and sends PIC EOI. AH=04h reads
the same work-area bytes; out-of-range groups return zero. Pending input does
not change either BIOS view until serviced, including while IF is clear or IRQ1
is masked. `Keyboard::pressed` remains the host's physical state, which can be
ahead of the BIOS when several events are queued. Repeated makes keep the bit
set and enqueue characters up to the existing 16-word ring limit; breaks clear
the bit without adding characters, even when the ring is full. Reading or
flushing the ring does not release held keys. An empty IRQ adds no character.
Command replies share the byte FIFO and remain readable at `41h`, but firmware
does not interpret them as scan input (including `FAh`/`A0h`, which overlap real
break codes). The existing `FFh` keyboard command clears queued bytes and replies
`FAh`, `A0h`; it preserves physical holds, the BIOS bitmap, modifiers and ring.
It is distinct from the full machine reset described below.

Guest DOS/TSR vectors still run first: chaining to the original INT 09 handler
updates the table; consuming bytes without chaining leaves BIOS state to the
guest handler. No scheduler or host-input shortcut writes the table. Machine
creation/reset initializes all 16 bytes to zero and discards held/pending keys,
modifiers and ring contents, consistent with the existing fresh-device reset.
This is the emulator's initialization policy; it does not reconstruct keys
physically held across a real hardware reboot.

## Graph LIO directory and GINIT

The original implementation in `src/lio.rs` follows the published interface in
the [NEC PC-9800 Technical Databook BIOS](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf),
printed pp.135, 138 and 141–146 (PDF pages 151, 154 and 157–162, zero-based).
The same scanned book is available from the [VTDA archive](https://vtda.org/docs/computing/NEC/PC-9800TechnicalDataBookBIOS%2BOCR_1992.pdf).
Only the interface description and diagrams were used; no NEC ROM instructions,
private work-area bytes or existing emulator implementation were used.

At physical `F9900h`, a four-byte header contains entry count `11h` and three
zero reserved bytes. Seventeen four-byte records follow: little-endian word ID
(`00A0h` through `00AFh`, then `00CEh`) and an offset relative to segment `F990h`.
The GINIT offset is therefore at `F990:0006`. A trailing ID-zero GRAPH BIO record
follows at offset 72 decimal. All records have distinct executable OUT/IRET
thunks at offsets `0100h` through `0144h`. The directory is ROM, rebuilt on reset.
It does not install LIO vectors: DOS programs must save/register/restore them.
The service is selected by the ROM entry reached, including when registered at
a different interrupt number; INT A0h itself is not intercepted.

GINIT takes a `1200h`-byte work area at `DS:0000`, and uses the guest's interrupt
stack. Callers should reserve the documented approximately 128-byte stack area.
It returns AH=00h and preserves DS, SS and SP. Both profiles support these defaults:

- Color graphics, 640×200, ordinary drawing, graphics display enabled. The fixed
  640×400 output doubles each graphics scanline; text retains its own scanlines
  and display state. This does not change the existing fixed scan timing.
- Active logical screen 0, display-screen selection 1, full inclusive viewport
  `(0,0)–(639,199)`. Display selection 1 means the first logical screen in hardware
  bank 0, not hardware display bank 1. CPU/display bank selectors become zero;
  bitmap start is zero, pitch 40 words, BIOS mode byte `0054Dh` is `40h` (LOWER)
  with bit 3 retaining the independently selected drawing timing.
- Palette mode 0 (eight digital colors), identity black/blue/red/magenta/green/
  cyan/yellow/white. The sixteen stored RGB values are initialized as in printed
  p.143, including gray and the seven dark colors for a later analog selection.
- Foreground 7, background 0, border 0; no charger/EGC/PEGC drawing mode. GINIT
  initializes the view and control state; it does not perform GCLS or erase VRAM.

Only common work `DS:0620h–069Fh` and command scratch `DS:1000h–11FFh` are
initialized. The documented caller-usable gaps and external guards are preserved,
including on repeated calls. The common bytes use an original opaque encoding:
at `0620h`, eight bytes hold mode, display switch, active screen, display selection,
palette mode, foreground, background and border; four little-endian words at
`0628h` hold viewport left, top, right and bottom. Remaining common/scratch bytes
are zero. This is an implementation detail, not an NEC-compatible private memory
layout or a promise that applications may access LIO-owned work bytes.

The supported buffer must fit completely within conventional RAM `00600h–9FFFFh`
and not overlap the six-byte interrupt frame. Invalid buffers fail with an explicit
`Machine.error` before GINIT changes work or display state; no undocumented AH
error value is invented. Every other LIO entry, including GRAPH BIO, similarly
stops with an explicit unsupported-service error. No drawing, GSCREEN, GCOPY,
monochrome/high-resolution LIO mode, or C5h drawing callback is implemented.

`guest/lioinit.asm` is the unchanged MIT Issue #24 control, source SHA-256
`53a6da02462395a8b5a56a9be33c7ab9f2f3278a38625e87a4344724c9ad23a3`.
`guest/liotest.asm` checks the directory, registers GINIT at INT 66h through DOS,
checks return registers, guards/gaps, scratch, palette and page state, and repeats
after dirtying hardware/work state. Its retained blue/red pixels distinguish the
first logical 200-line screen and doubled rows from hidden VRAM in native and
production-Wasm framebuffer checks. The native test observes every slot of a
16-million-slot DOS run: exactly one reset-vector entry and three AH=00h returns.
Browser tests run the same bounded control twice across a reset at both hosting
paths and on both profiles. No claim of TRIANS98 initialization beyond GINIT,
file creation, drawing, gameplay, or save/reload follows from this control.

## Accepted software and media

- MIT HelloWorld-PC98: exact upstream first boot sector on an otherwise zeroed
  77×2×8×1024 disk; text VRAM writes and PIT beep prove independent guest execution.
- Original MIT Star Trail: boot loader reads a second stage, uses direct text/bitmap
  VRAM, BIOS keyboard/beep and sector writes. Gameplay/save assertions inspect
  guest-owned record `1FC0:1800` (`PC98GAME`) and saved sector 8 (`STAR98SV`).
- FreeDOS(98) kernel build 2043 DBCS, 2022-01-20, plus FreeCOM 0.85a DBCS,
  2022-01-17: minimal real-mode floppy reaches `A:\>`, runs original PROBE.COM
  through DOS and returns to FreeCOM. XMS drivers and unrelated utilities are
  excluded. The original full distribution's 286 XMS driver cannot install on
  this profile; no protected mode compatibility is claimed.

`src/disk.rs` validates raw sizes and D88 headers/track offsets/sector counts,
CHRN/length bounds, write protection, normal MFM status and duplicate CHR records.
Only one disk per D88 is supported. Track data must exactly fill the declared track
boundary; unusual/padded/damaged/deleted/non-MFM sectors fail explicitly.
[D88 layout reference](https://www.pc98.org/project/doc/d88.html).

All other software remains unvalidated. In particular, an arbitrary period game
may need missing instructions, BIOS services, Kanji, sound or graphics commands.
The browser and headless command expose failures; no compatibility workaround
recognizes a guest filename or substitutes host-rendered output.

## BIOS interval callback (INT 1C/AH=02)

The normal-mode single-event service follows the manufacturer's
[PC-9800 BIOS databook, printed p.13](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
and the [PC-9801 programmer reference, printed p.52](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf).
CX specifies 10 ms units; CX=0 means 65,536 units (655.36 seconds). ES:BX is
the guest interrupt routine. All request registers are preserved (the databook
only guarantees registers other than AX). The callback must preserve the
registers it uses and return with IRET, as required by the programmer reference.

There is one active request. A subsequent AH=02 request replaces its interval
and address, even if the previous expiry is waiting for interrupt delivery.
This is the emulator's deliberate replacement contract for the single-event
service: the references describe one interval/address and distinguish it from
the separate high-resolution multievent services, but do not specify overlapping
AH=02 requests. There is no queue or implicit periodic callback. A callback may
issue AH=02 again; its previous request is disarmed before guest entry.

Scheduling uses the existing 8 MHz virtual clock, with expiry at request time
plus CX × 80,000 cycles, independent of host time and step slice size. Expiry
latches IRQ0 and waits for IF, STI/MOV SS interrupt shadows, PIC masking and
priority. The synthetic IRQ0 handler acknowledges the PIC and transfers to
ES:BX with its real six-byte FLAGS/CS/IP interrupt frame. Guest instructions
execute the handler; IRET restores the interrupted address, stack and FLAGS.
No instruction executes early; delivery may be late while interrupts are gated.
Reset clears both armed and expired requests. The existing PIT programming and
periodic IRQ0 tick word at 058Ah remain available; AH=02 still clears that word.
This is a deterministic BIOS service, not a change to PIT clock/mode fidelity.
Guest replacements for IRQ0 must chain to the BIOS to receive this service.
High-resolution AH=03/04/05 timer services remain unsupported.

## BIOS drawing timing (INT 18/AH=4A)

The Ap DOS profile supports CH=06h (flash drawing) and CH=16h (flashless drawing),
following the [NEC BIOS databook, printed p.91, PDF index 107](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf).
Flash permits drawing during active display; flashless requests drawing during
blanking. AL and CL are not inputs. The synthetic service preserves all general
and segment registers, including upper words and FS/GS, and FLAGS. The executable
ROM thunk returns through IRET to the instruction after INT 18h with the original
stack and caller flags.

The request is stored as `gdcs[1].draw_during_display` on the bitmap GDC and as
bit 3 of the BIOS work byte `0000:054D` (PRXDUPD): one for flash, zero for
flashless. This work-area encoding is described in the
[BIOS work-area reference](https://www.webtech.co.jp/company/doc/undocumented_mem/memsys.txt).
Other work-byte bits and text GDC state are preserved. AH=42 graphics mode setup
preserves the timing bit; display on/off also leaves the request intact. Machine
creation/reset chooses flashless (false/bit 3 clear), a deterministic emulator
default rather than a claim about every physical machine's power-on setting.
Direct guest RAM writes to the work byte do not program the GDC.

This is a timing **selection** contract. Both choices use the unchanged fixed
8 MHz scheduler, horizontal/vertical status periods and instantaneous VRAM
writes. Selection does not advance clocks, wait for blanking, restart scan phase,
change display enable, consume the GDC FIFO or alter rendered pixels. The core
has no GDC vector/area drawing engine, drawing-busy interval, display-memory
contention, visible flash artifact or mode-dependent drawing throughput.
Physical Ap machines can remain flashless regardless of this request; this
subset records the requested mode and does not add a hardware capability flag
or simulate that arbitration. No physical speed or flicker accuracy is claimed.

Only the two documented CH values are accepted on Ap. Other CH values stop with
an explicit unsupported-drawing-timing core error before changing state; no
undocumented guest status code is invented. VM continues to reject AH=4A with
its existing unsupported-BIOS diagnostic. Expanding the preserved VM profile is
outside Issue #23; supporting Ap does not assert a VM drawing engine exists.

`tools/prepare_freedos.py` builds separate `drawmode.hdm` and `drawtest.hdm` disks
using the existing unmodified, checksum-pinned FreeDOS kernel and FreeCOM.
The exact [original control](../guest/drawmode.asm) has source SHA-256
`9c4e0fb1d0a043e5cc7b0978b53be2be89431df6a5d74af23637c5630bfdc792`.
The [switching probe](../guest/drawtest.asm) checks all 16-bit general/segment
registers, SP, FLAGS and the work byte through actual INT/IRET, exercising
06h → 16h → 16h → 06h. It publishes observations at `9000:0000` (stage) and
`9000:0001..0004` (work-byte snapshots), then waits for a keyboard event at each
stage. Native and production-Wasm browser tests inspect the snapshots and live
work byte, inject Enter, require `DRAW TIMING PASS`, `PROBE SHELL RETURN` and
the DOS prompt, and verify reset and reboot. Each wait is capped at 16 million
scheduler slots. Native service tests additionally check upper registers, all
unsupported CH values, both profiles, internal GDC state and unchanged scan/FIFO
state. Neither fixture requires a game, proprietary BIOS or font, or is published
in `web/`. Successful return establishes this BIOS contract only; PIPOO
playability and any later game stops remain unvalidated.
