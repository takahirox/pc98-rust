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
string operations and REP/REPE/REPNE, plus [REPC/REPNC comparisons](#v30-repcrepnc),
interrupt entry/IRET, byte/word IN/OUT,
CLI/STI/HLT. Immediate push, PUSHA/POPA, ENTER/LEAVE and immediate shifts implement
the 186-compatible forms available on V30. TEST1/CLR1/SET1/NOT1 implement V30
single-bit operations; [memory nibble rotations](#v30-memory-nibble-rotations)
implement `ROL4 mem8` and `ROR4 mem8`. [ADD4S](#v30-add4s) adds packed-BCD
strings. BOUND/CHKIND implements signed inclusive word bounds and next-instruction
vector-5 return as [specified below](#v30-boundchkind). AAM/AAD
consume but ignore the encoded radix, using decimal 10 as on V30. See the [NEC V20/V30 user manual](https://www.dosdays.co.uk/media/nec/NEC-V20-V30-Users-Manual.pdf)
and [NEC V-Series data book](https://bitsavers.org/components/nec/_dataBooks/1990_NEC_16-bit_V-Series_Microprocessor_Data_Book.pdf).

Unimplemented opcodes fail with CS:IP and opcode information. V30 8080 emulation
mode, remaining packed BCD strings (SUB4S/CMP4S), bit-field extensions, coprocessor/ESC,
TF single-step traps, NMI, undocumented opcodes and later x86 extensions are not
implemented in VM. LOCK has no bus-contention effect in this single-CPU machine.

The selectable Ap profile extends this same decoder with 32-bit operands and
addresses, SIB, FS/GS and selected 386/486 real-mode integer instructions. It checks
64 KiB segment limits and delivers guest exceptions; it does not provide protected
mode, DPMI or numerical FPU execution. See the [exact Ap CPU coverage and independent DOS diagnostic](dos-compatibility.md#issue-9-cpu-coverage-and-independent-guest-evidence).
VM retains the V30 behavior described above.
Ap also provides the [real-mode GDTR](#ap-real-mode-gdtr) and
[IDTR prerequisites](#ap-real-mode-idtr), plus
[SMSW status readback](#ap-real-mode-smsw) and
[bounded x87 initialization/readback](#ap-x87-environment-initialization) below.

An instruction or one REP element consumes one 8-clock scheduling slot. This is a
deterministic work/cycle model, **not instruction-cycle accuracy**. Firmware FDC
transactions additionally advance the same device scheduler. HLT stops instruction
retirement while clocks and IRQs continue. STI and MOV/POP SS inhibit delivery for
the next instruction on both profiles; VM also inhibits after the other documented
[segment MOV/POP forms](#v30-segment-interrupt-inhibition). A frame runs at most
two slices of 16,667 slots; the core caps each call at 100,000 slots. There is no unbounded catch-up after a suspended
tab. This bounds UI work even for infinite loops and maximum-length REP strings.

## V30 segment interrupt inhibition

VM delays maskable INT acknowledgement for the next instruction after documented
segment MOVs in both directions and segment POPs. Destinations are ES/SS/DS for
MOV-to-segment and POP, and sources are ES/CS/SS/DS for MOV-from-segment.
MOV accepts register and memory operands. The existing interrupt inhibition counter protects
one boundary; each consecutive qualifying instruction renews it. Prefixes belong
to their instruction. IF, operand transfers and FLAGS are preserved; the PIC
request remains pending until an eligible boundary. Updating SS protects the
following stack-pointer setup, so delivery uses the new SS:SP.

Specification: NEC's [V20/V30 User's Manual, Chapter 6, printed page 6-4,
“Interrupt Disable Timing”](https://ftpmirror.your.org/pub/misc/bitsavers/pdf/nec/V20_V30_Users_Manual_Oct86.pdf#page=29).
Ap retains its existing SS/STI interrupt shadows and TF behavior; its non-SS
segment MOV/POP instructions gain no inhibition. Undocumented MOV-to-CS/POP-CS
forms are outside this contract.

The original MIT [probe](../guest/v30_segment_irq.asm) establishes pending PIT
requests with IF clear, confirms PIC IRR through guest I/O, executes STI followed
by the producers and an ordinary INC, then verifies ordering inside the handler.
It checks exact return IP/CS, the six-byte frame and saved IF, one entry per case,
ISR/IRR and EOI/IRET, full results and guards before reporting PASS. The checked
[manifest](../fixtures/v30-segment-irq-manifest.json) pins offline provenance and
hashes. Native and production release-Wasm tests boot the same image at `/` and
`/pc98-rust/`, before and after reset, within 2,000,000 slots per boot. Hosts only
insert, reset, step and read; no host-written results or injected interrupts
supply guest acceptance. See the [validation record](validation.md#issue-69-v30-segment-interrupt-inhibition-2026-10-24).

Acceptance is bounded to maskable interrupts and ordinary non-REP following
instructions. Existing REP interruption regressions remain required, but do not
verify all segment-shadow/REP interactions. V30 remains **partial**: remaining
packed-BCD strings (SUB4S/CMP4S), bit-field extensions, TF traps, NMI and broader
instruction/prefix/fault acceptance remain outstanding. Protected execution, paging and basic x87 retain
their required scope. No inventory category is completed and no full V30 accuracy
or commercial compatibility is established.

## V30 ADD4S

VM decodes `0F 20` as two bytes without ModR/M. For valid packed-BCD operands,
CL=1..254 selects `ceil(CL/2)` bytes: source defaults to DS:SI and permits ES/CS/SS/DS
overrides; destination is fixed ES:DI. Both digits of the final byte participate
for odd CL. Incoming CF is ignored; decimal carry propagates across the rounded
width. CF reports overflow beyond that width and ZF reports an entirely zero
rounded result. Thus CL=4, 9999+0001 gives 0000 with CF=ZF=1, while CL=3,
0999+0001 gives 1000 with CF=ZF=0. A nonzero extra high digit also participates.

Specification: NEC's [V-Series Instruction Manual, printed pp.15–16](https://www.ardent-tool.com/CPU/docs/NEC/V20-V30/v_series.pdf#page=26).
Increasing byte offsets, unchanged SI/DI/CL and DF-independent traversal are our
interpretation of the operation and offset diagram; they are not separately
stated hardware guarantees. NEC describes DIR-controlled traversal separately
for block transfer/I/O in [Appendix A.4(9)](https://www.ardent-tool.com/CPU/docs/NEC/V20-V30/v_series.pdf#page=200).
Other registers and control flags are preserved. AF/OF/PF/SF are hardware-undefined;
**preserving their initial values is deterministic emulator policy**, tested
separately from defined status results.

Each byte uses existing CPU memory helpers and Bus mappings, including separate
16-bit source/destination offset wrapping, 20-bit physical wrapping, ROM read/write
protection and ordinary planar VRAM with accelerators disabled. Disjoint buffers
and exact aliasing are tested. Ap still takes its separate i486 extended decoder
and reports `unimplemented i486 opcode 0F 20` without arithmetic side effects;
that is an emulator limitation for the control-register family, not an architectural
#UD claim.

The independent base-100 Rust oracle exhausts all 100×100 one-byte operand pairs
with both incoming CF states and covers every documented CL with deterministic
patterns, full and terminated carry chains, leading zeros and whole-result ZF.
The original MIT [guest probe](../guest/v30_add4s.asm) checks five literal sums,
masked flags, unchanged source/registers and guards itself. Its checked
[manifest](../fixtures/v30-add4s-manifest.json) pins offline assembly, license,
source/boot/image hashes, exact results and a finite 2,000,000-slot budget.
Native and production release-Wasm tests boot the identical VM image cold and
after reset, at `/` and `/pc98-rust/`. Hosts only mount, reset, step and inspect.
See the [actual validation record](validation.md#issue-95-v30-add4s-2026-10-24).

CL=0/255, nondecimal nibbles, partially overlapping buffers, new REP/LOCK semantics,
interruption within ADD4S and exact instruction timing remain outside this verified
subset and remain future inventory work. The operation currently completes in one
scheduler slot; this is not an instruction timing guarantee. V30 remains **partial**:
SUB4S/CMP4S, bit-field extensions, register nibble rotations, TF traps, NMI and
broader flags/prefix/interrupt/fault acceptance remain outstanding. All 26 category
scopes/statuses remain unchanged; protected execution, paging and basic numerical
x87 remain required. These probes establish neither full CPU accuracy nor commercial
compatibility.

## V30 memory nibble rotations

VM supports memory-only `ROL4 mem8` (`0F 28 /0`) and `ROR4 mem8` (`0F 2A /0`),
without an immediate operand. With initial memory byte `d` and `a = AL & 0Fh`:

| Instruction | New memory byte | New AL low nibble |
| --- | --- | --- |
| ROL4 | `((d << 4) & F0h) \| a` | `d >> 4` |
| ROR4 | `(a << 4) \| (d >> 4)` | `d & 0Fh` |

Both inputs are read before either output changes. Exactly one byte goes through
the existing effective-address, segment and Bus paths, including ROM protection,
planar VRAM, 16-bit effective-offset wrapping and VM 20-bit physical wrapping.
FLAGS, AH and other registers remain unchanged. NEC leaves AL's upper nibble
unspecified; **preserving it is a deterministic emulator policy**, separately
tested from the hardware-defined nibble mapping.

Specification: NEC's [V-series Instruction Manual, printed pp.136 and 141](https://www.ardent-tool.com/CPU/docs/NEC/V20-V30/v_series.pdf#page=147)
and [V20/V30 User's Manual, Section 12, printed pp.12-45–12-46](https://www.dosdays.co.uk/media/nec/NEC-V20-V30-Users-Manual.pdf).
Register destinations (including AL overlap) and nonzero ModR/M.reg encodings
retain explicit unsupported emulator-limitation diagnostics without operand
writes. Ap retains its existing `unimplemented i486 opcode` diagnostic for these
bytes. These diagnostics are scope boundaries, not hardware exception claims.
This change adds no LOCK/REP behavior or instruction-cycle accuracy.

The original MIT [probe](../guest/v30_nibble_mem.asm) and checked
[manifest](../fixtures/v30-nibble-mem-manifest.json) provide offline provenance,
hashes, exact masked-AL/AH/FLAGS/results/guards and a 2,000,000-slot boot budget.
Native and production release-Wasm tests boot the same VM disk before/after reset
at both browser roots. Hosts only insert, reset, step and read. See the
[validation record](validation.md#issue-65-v30-memory-nibble-rotations-2026-10-24).
V30 remains **partial**: register-operand nibble rotations, remaining packed-BCD
strings (SUB4S/CMP4S), bit-field extensions, TF traps, NMI and broader
instruction/prefix/fault acceptance
remain outstanding. Protected execution, paging and basic x87 retain their fixed
required scope. No inventory category becomes verified; bounded probes do not
establish full hardware accuracy or commercial compatibility.

## V30 REPC/REPNC

On VM, `65h` is REPC and `64h` is REPNC for CMPSB, CMPSW, SCASB and SCASW.
CX zero skips comparison, leaving flags and indices unchanged. Otherwise each
step compares one element, adjusts SI/DI according to width and DF and decrements
CX without changing comparison flags. CMPS subtracts destination from source;
SCAS subtracts destination from AL/AX. The source uses DS or its segment override;
the destination always uses ES. VM word accesses and indices wrap at 16-bit
offsets. Incoming CF never suppresses the first comparison. Subsequent elements
require nonzero CX and resulting CF set for REPC or clear for REPNC; ZF does not
control repetition. These semantics follow NEC's
[V-Series instruction manual, printed pp.126–127](https://www.ardent-tool.com/CPU/docs/NEC/V20-V30/v_series.pdf#page=137),
with CMPBK/CMPM on printed pp.61–64 corresponding to CMPS/SCAS.

Prefixes and opcode decode in the same step. A continuing iteration restores IP
to the first prefix, so a PIC interrupt between elements saves that restart IP;
guest IRET resumes with the remaining CX and updated indices. Scheduling remains
one element per slot, without instruction-cycle accuracy changes. Native tests
cover full FFFFh exhaustion, flags, guards, offset wrapping and PIC/IRET resumption.
The [original MIT probe](../guest/v30_repc.asm) and
[manifest](../fixtures/v30-repc-manifest.json) establish bounded native/release-Wasm
boot and reset evidence. Hosts only boot, step and read results.

Carry-repeat with non-compare opcodes or conflicting repeat prefixes produces an
explicit unsupported emulator-limitation diagnostic. This describes emulator scope
and does not claim hardware exception semantics for those excluded encodings.
Ap continues to decode `64h`/`65h` as FS/GS overrides, including with ordinary
F2/F3 repetition and operand/address-size overrides. V30 remains partial:
remaining packed-BCD strings (SUB4S/CMP4S), bit-field extensions, VM TF traps, NMI
and broader instruction, prefix and fault acceptance remain outstanding. Protected execution, paging and
x87 remain required project goals outside this Issue. These probes do not establish
full V30 accuracy or commercial compatibility.

## V30 BOUND/CHKIND

VM implements `62 /r`, comparing a signed 16-bit register against two consecutive
little-endian memory words, lower then upper, with inclusive endpoints. Valid
checks preserve registers, flags and bounds. Existing ModR/M decoding supplies
16-bit offset wrapping, default SS for BP addressing, segment overrides and Bus
mapping. Register-only operands retain an explicit host diagnostic.

An out-of-range VM value conditionally enters vector 5 regardless of IF. The
fixed physical IVT entry at `00014h..00017h` supplies IP/CS. Existing interrupt
entry pushes original FLAGS, CS and the decoded **next-instruction IP**, then
clears IF/TF. Unmodified IRET continues after BOUND. Ap retains its existing
16/32-bit signed comparison and restartable fault-5 behavior: its saved address
identifies the first prefix. Ap operand/segment-limit and LOCK rejection paths
are unchanged.

Specification basis: NEC's [V-series Instruction Manual, printed pp. 52–53 and
Appendix A.4, p. 185](https://www.ardent-tool.com/CPU/docs/NEC/V20-V30/v_series.pdf#page=63)
defines the two-word operand, conditional BRK 5, vector and stack operations,
and increments PC as instruction bytes are consumed. **Next-instruction return
is the interpretation of those combined documented operations.** NEC's
[V20/V30 User's Manual, Section 6](https://www.ceibo.com/eng/datasheets/NEC-V20-V30-Users-Manual.pdf)
describes interrupt entry. Intel's [80386 BOUND specification](https://www.scs.stanford.edu/05au-cs240c/lab/i386/BOUND.htm)
explicitly specifies signed inclusive comparisons and a restart address; that
Ap fault rule must not be applied to VM.

The original MIT [boot probe](../guest/v30_bound.asm) builds offline with
`python3 tools/prepare_v30_bound_fixture.py`. Its checked
[manifest](../fixtures/v30-bound-manifest.json) pins source/license, assembly,
source/boot/image hashes, exact results/guards and a two-million-slot limit.
The guest checks both vector-5 frames, IF/TF clearing, unmodified IRET and exactly
two entries with forward progress. Native and production-release Wasm checks
boot the identical VM image before/after reset, at both browser roots. See the
[actual validation record](validation.md#issue-57-v30-boundchkind-2026-10-24).

V30 remains **partial**: remaining packed-BCD strings (SUB4S/CMP4S), bit-field
extensions, TF single-step, NMI and broader instruction/fault acceptance remain
outstanding, as do the other
VM exclusions above. Protected execution, paging and basic x87 retain their
fixed required scope. These bounded probes establish neither full hardware
accuracy nor commercial compatibility.

## Ap x87 environment initialization

Ap implements only `DB E3` FNINIT, `D9 /7` memory FNSTCW, `DD /7`
memory FNSTSW and `DF E0` FNSTSW AX in real mode. The environment belongs to
one CPU. FNINIT sets control `037Fh`, status `0000h`, tags `FFFFh` (all empty),
and clears instruction/data pointers, selectors and last opcode. Stack data is
not modeled or zeroed. Stores read current words, always write two bytes, and
preserve EAX's upper half for AX. Integer flags and other registers are preserved.
These forms never check pending numerical exceptions and never replace saved
pointers or opcode with their own addresses. The stores' undefined x87 condition
bits are preserved as a deterministic emulator policy, not a hardware guarantee.
See Intel's [SDM Volume 2A, printed pp. 3-395–396 and 3-438–443](https://cdrdv2-public.intel.com/782150/253666-sdm-vol-2a.pdf).

Either CR0.EM or CR0.TS causes restartable exception 7. LOCK causes exception 6.
Memory offsets outside `0000h..FFFFh`, including a word starting at `FFFFh`, cause
exception 13 **even for SS**. This deliberately follows the target-era
[Intel486 PRM, printed pp. 26-97–98, 26-132 and 26-135–136](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143),
rather than the modern SDM's SS-overrun table. Both bytes are checked before any
write; normal bus A20 mapping and ROM protection still apply. Fault frames use
the first prefix and the current IDTR. No simultaneous-fault priority is promised.

Cold construction deterministically supplies initialized metadata. `Machine::reset`
models RESET without BIST and **retains** this environment until guest FNINIT;
Intel486 PRM §19.2.3 distinguishes that reset from BIST/FNINIT initialization.
This policy does not claim hardware power-up metadata is defined.

The original MIT [guest probe](../guest/fpu_init.asm), offline
[builder](../tools/prepare_fpu_init_fixture.py) and hash-pinned
[manifest](../fixtures/fpu_init-manifest.json) check all four forms, prefixes,
exact widths/guards, boundary stores, guest #GP/#UD repair and IRET/retry.
Native and production release-Wasm browsers boot the same image cold and after
reset at `/` and `/pc98-rust/`, within 2,000,000 slots. Native tests additionally
seed every field and independently exercise EM/TS and relocated delivery.

FPU inventory status is **partial**, with no scope verified. Stack/load/store
operations, arithmetic/comparisons/conversions, rounding/precision, other control
and environment operations, numerical exceptions, WAIT/FWAIT completion and
NE/IRQ routing remain required. Other valid x87 forms stop explicitly unsupported.
VM coprocessor behavior, Ap CPUID rejection and optional FPU-detection fixture
adaptations remain unchanged. Protected execution, paging and other inventory
gaps remain required; these limited probes establish neither complete hardware
accuracy nor commercial compatibility.

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
LMSW, CLTS, protected execution, paging and numerical x87 execution remain unimplemented.
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

## Ap calendar read path

Ap implements the bounded µPD4990A register-control protocol at `20h` and `33h`,
based on NEC's [PC-9800 Technical Data Book HARDWARE, printed pp.127 and 137](https://vtda.org/docs/computing/NEC/PC-9800TechnicalDataBookHARDWARE%2BOCR_1993.pdf#page=140)
and [µPD4990A IC-1755 datasheet, printed pp.4–8 and Figures 2–5](https://www.renesas.com/en/document/dst/upd4990a-data-sheet).
Port `20h` has C0–C2 in bits 0–2, STB in bit 3, CLK in bit 4 and DATA IN in bit 5.
Port `33h` retains its existing `38h` inputs and adds DATA OUT in bit 0.

STB rising latches parallel `000` Hold, `001` Shift, `010` Time Set or
`011` Time Read. With C0–C2 continuously `111`, CLK rising shifts DATA IN into
the independent four-bit command register, least significant bit first; STB rising accepts serial `0000`,
`0001`, `0010` or `0011` for those same functions. Repeated unchanged control
writes have no edge and no effect. Electrical setup/delay tolerances are not
simulated.

Time Read captures the running counter into a held shift register. Hold freezes
that register; neither command stops the calendar. Shift immediately exposes the
initial LSB, then CLK rising shifts to the next bit, which is stable at CLK's
trailing edge for CPU sampling (Figures 4–5). Parallel transfers have 40 calendar
bits; serial transfers have 48 calendar bits followed by four command bits.
In serial mode the command register shifts on every CLK rising, even during
Hold/Time Set/Time Read. During Shift its old LSB feeds the top of the 48-bit
calendar register, with DATA IN entering the command register. The command tail
therefore contains the command bits present when shifting began, unless subsequently
replaced by further shifting. In parallel mode DATA IN feeds bit 39 directly.

Calendar bytes are seconds, minutes, hours, day, month/weekday, then year in
extended transfers. Bytes are BCD except hexadecimal month in the high nibble;
weekday occupies the low nibble, Sunday=0. For example, seed year=86, November 25,
weekday=6, 02:23:49 yields `49 23 02 25 B6` in compatibility mode and
`49 23 02 25 B6 86` plus command nibble `1` after serial Shift framing. All bits
are least significant first. A new Time Read replaces a partial snapshot; elapsed
seconds never alter an already captured snapshot.

The counter uses 24-hour time, month lengths and weekday rollover. Parallel
commands disable year counting and automatic leap days: February normally has
28 days, while an explicitly loaded February 29 rolls to March 1. Serial commands
enable the two-digit year (99→00) and leap years divisible by four, including 00.
This follows the chip's two-digit rule, without Gregorian century correction.
Cold construction starts in compatibility Hold at 2026-01-01 Thursday 00:00:00.
`Rtc::new(Calendar::new(year, month, day, weekday, hour, minute, second)?)` supplies
an explicit validated binary seed for native tests; it is not a guest setting
service. Each Bus owns its own RTC.

While running, every 8,000,000 scheduler clocks advance one second. Fractional
clocks survive all tick batch sizes, including `u32::MAX`, and advancement continues with IF
clear or HLT. Hold/Time Read DATA OUT is a 1 Hz, 50% duty waveform, deterministically
low for the first half-second and high for the second. Register commands preserve
phase except Time Set entry/release, as described below. Host wall time and
browser animation timing do not initialize or advance the counter.
`Machine::reset` retains the entire battery-backed RTC: calendar, fraction, active mode/width, control levels, captured data and command register.
A partial transfer can continue across reset; a rebooting guest should frame a
fresh command/capture itself.

Time Set transfers the shift register to the counter on STB rising: 40 parallel
bits preserve the stored year; 48 serial bits load the year too. To serially load
while in Shift, send the six calendar bytes followed by the four-bit Time Set
command, all LSB first, then strobe. The complete chain is 52 bits: clocking
another command after all 52 bits would displace the calendar data.
Time Set holds the calendar counter and shift register and drives DATA OUT from
the held LSB. CLK can still shift the independent serial command register.
Commands 0/1/3 release the counter and perform their usual register operation.

Entry (including repeated Time Set strobes) and release reset divider stages
10–15; lower stages continue. At the 8 MHz scheduler rate their retained phase
is the position within a 125,000-clock (64 Hz) period. The next second occurs on
the 64th subsequent 64 Hz edge after release, rather than necessarily 8,000,000
clocks later. Held time cannot accumulate catch-up seconds. Machine reset while
held retains that state and lower phase; it does not implicitly release Time Set.

Valid BCD/range/date payloads load atomically, including an explicitly loaded
parallel February 29 even when the stored year is not a leap year. Malformed
BCD, impossible dates or out-of-range fields retain the entire previous calendar;
Time Set still applies hold, LSB output and divider controls. This is a bounded
emulator policy, not a claim about unspecified invalid-data silicon behavior.
The original [Time Set guest](../guest/rtc_set.asm),
[manifest](../fixtures/rtc-set-manifest.json) and
[validation evidence](validation.md#issue-87-ap-rtc-time-setcounter-hold) cover
this contract. Limited probes and game regressions establish neither full RTC
accuracy nor commercial compatibility.

Ap INT `1Ch/AH=00` copies one coherent snapshot to ES:BX as year, month/weekday,
day, hour, minute, second. It retains the existing 16-bit destination-offset
wrapping and caller register/flag behavior. VM retains its fixed BIOS date and
port inputs. AH=02 remains the independent interval callback service.

RTC is **partial**. BIOS setting services, TP selection,
interval/IRQ controls, test mode and the VM's older µPD1990-family behavior remain
outstanding. Unsupported RTC commands currently leave the implemented register
mode unchanged. General 8255 emulation and host synchronization are absent.
The [original MIT guest](../guest/rtc.asm), [checked manifest](../fixtures/rtc-manifest.json)
and [validation record](validation.md#issue-61-ap-calendar-reads-2026-10-24) cover
this bounded read behavior, without establishing whole-category accuracy.

## Device ports and interrupts

Port facts are traced to the individual chapters of
[UNDOCUMENTED 9801/9821 Vol.2](https://www.webtech.co.jp/company/doc/undocumented_mem/index.html).
Registers below implement the behavior needed by the three accepted workflows;
this table is not a claim of complete chip emulation.

| Ports (hex) | Device and supported behavior | IRQ / notes |
| --- | --- | --- |
| `00,02`; `08,0A` | Master/slave 8259: SNGL/IC4 initialization, masks, IRR/ISR, OCW2 programmable/rotating priority, specific/nonspecific EOI, auto EOI, automatic rotation, OCW3 special-mask mode and normal-EOI polling | Master IR7 cascades slave unless SNGL selects local delivery; each controller resolves its own priority and ISR blocking |
| `01–0F` odd, `11–1F` odd, `21,23,25,27` | 8237 address/count flip-flop, controller disable, channel masks, direction/decrement, terminal count and auto-init reload, master clear, bank registers | FDC uses channel 2; count/address change with actual byte transfers |
| `31,33,35,37` | PPI startup inputs, output C, bit set/reset, active-low beep gate | Input straps are fixed VM-profile constants; Ap RTC DATA OUT replaces bit 0 of 33h |
| `41,43` | Keyboard byte FIFO, ready status, command acknowledgements, down/up scan bytes | IR1 / INT 09; firmware maintains BIOS ring, held-key bitmap and modifier state |
| `60,62` | Master text GDC parameter/status and command/data readback | RESET, display enable, PITCH, CURS, CCHAR, PRAM start, CSRR |
| `A0,A2` | Slave bitmap GDC command/parameter/status/readback | Same register subset; 640×400 direct VRAM path |
| `68` | Text display enable; graphics 200/400-line selection | Fixed 80-column 25-row text; 200-line graphics doubles rows |
| `6A,A8,AA,AC,AE` | Digital 8-color pairs; analog index and 4-bit RGB levels when enabled | Eight palette entries only; no fourth graphics plane / 16-color board |
| `A4,A6` | Display and CPU graphics page select | Two independent three-plane pages |
| `7C,7E` | GRCG mode/tile writes | TDW and RMW, plane masks; no EGC |
| `71,73,75,77`; `3FD9–3FDF` odd | 8253 channels, control/access modes, low/high writes, counter latches | Channel 0 IR0 / INT 08; channel 1 beep; VM input clock 1,996,800 Hz |
| `90,92,94` | 2HD FDC status/FIFO/control, SPECIFY, sense status/interrupt, recalibrate, seek, read ID, READ/WRITE DATA through DMA, single-head non-DMA READ/WRITE DATA, board-control gating | IR11 / INT 13; delayed command completion and seven-byte transfer results |
| `BE` | Fixed 1 MB floppy interface selection | No alternate 640 KB controller at C8/CA |
| `40,5C,5E,64,6E` | Fixed system/display inputs and deterministic clock status | See `Bus::input`; VM has no RTC serial protocol |
| `F0` | Private synthetic-firmware thunk ABI, accepted only from its ROM stubs | Not a guest DOS API or claim of a real VM hardware port |

### 8259A single-mode initialization and 8086 delivery

At both controllers' command/data ports, a fresh ICW1 (D4=1) restarts
initialization at any stage. ICW1.SNGL (D1) controls ICW3 consumption;
ICW1.IC4 (D0) controls ICW4 consumption:

| SNGL | IC4 | Initialization data writes after ICW1 |
| --- | --- | --- |
| 0 | 0 | ICW2, ICW3 |
| 0 | 1 | ICW2, ICW3, ICW4 |
| 1 | 0 | ICW2 |
| 1 | 1 | ICW2, ICW4 |

The next and all subsequent data writes are OCW1 masks. ICW2 retains bits 7:3;
the supported 8086 vector is `(ICW2 & F8h) | local_irq`. ICW4.D1 selects AEOI:
normal acknowledgement consumes IRR and retains ISR until EOI, while AEOI
consumes IRR without retaining ISR. IC4=0 leaves AEOI disabled; acceptance of
that sequence does not implement MCS-80/85 interrupt output.

ICW1 preserves the existing initialization policy: clear IRR/ISR/IMR, disable
AEOI/rotation/special-mask mode, restore fixed priority and IRR readback, and
cancel any armed poll. Construction and Machine reset retain the existing
masked, cascade-mode defaults and vector bases 08h/10h. Programming one
controller does not initialize the other.

SNGL on the master makes IR7 a local acknowledgement returning its own vector;
it does not acknowledge the slave or drop its physical INT history. Fixed
slave INT → master IR7 wiring still captures rising edges, including while IF
is clear. Held slave INT remains held across local acknowledgement, master EOI,
ticks, port operations and PIC initialization, so it cannot repeatedly latch
IR7. An observed deassertion followed by reassertion captures a new edge.
Software can separately poll/acknowledge the slave. With quiescent inputs,
reinitializing ordinary cascade mode restores slave vector delivery and separate
normal-EOI obligations on both controllers.

The reference is [Intel 8259A, interrupt sequence/vector format and initialization
Figures 6–7, printed pp. 7–11](https://www.scs.stanford.edu/11wi-cs140/pintos/specs/8259A.pdf#page=7).
Original [native tests](../tests/pic.rs) cover all four sequences at both port
pairs, masks, vector alignment, all local IRQs, normal/AEOI delivery, held/new
slave INT edges, reinitialization/restarts, poll cancellation, isolation and reset.
The original MIT [guest probe](../guest/pic_single.asm) uses guest PIT/FDC I/O
and real hardware interrupt entry/IRET to check single-mode normal/AEOI PIT,
local IR7 with an untouched pending slave, held INT, separate slave drain, then
restored cascading. The [manifest](../fixtures/pic-single-manifest.json) pins
source/license, assembly command, source/boot/image hashes, 64 exact result/guard
bytes and the 2,000,000-slot budget. The same image is checked natively and in
production release-Wasm, on VM/Ap before/after reset and at both browser roots.
See [validation evidence](validation.md#issue-91-8259a-single-mode-initialization-and-delivery-2026-10-24).
PIC remains partial; the limitations listed below still apply.

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
ICW sequencing honors SNGL and IC4 as specified above; physical master-IR7
wiring is retained.
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

### 8259A OCW3 normal-EOI polling

With ICW4.AEOI=0, OCW3.P (`0Ch`) arms one read acknowledgement at the
controller's existing ports. A successful read returns `80h | local_level`,
clears that IRR bit and sets its ISR bit; the vector base does not enter the
result. Empty polls change no service bits. Unspecified result bits read zero
as a deterministic emulator policy. The selection (including an empty result)
is frozen at the poll write, using the current masks, priority origin and ISR
blocking, including special-mask mode. Later requests remain pending for later
service; repeated requests coalesce in the existing IRR latch.

The next controller read consumes the transaction, including a data-port read.
Polling overrides combined P/RR/RIS status commands; RR/RIS still selects the
ordinary status register for subsequent reads. A fresh poll command is required
for each acknowledgement. Without an armed poll, status and IMR reads remain
passive. ICW1 cancels a transaction; construction and Machine reset clear it.
Polling leaves INT arbitration active: software must disable CPU interrupts.
The reference is [Intel 8259A, Figure 8, Poll Command, Reading Status and Figure 9,
printed pp. 13–17](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=13).

In the fixed PC-98 cascade, rising slave INT transitions latch master IR7 during
bus ticking and PIC port operations, independently of CPU IF. An asserted slave
INT is not repeatedly latched while software performs the two reads. Master
polling acknowledges only master IR7; the guest must separately poll the slave
for its local level and separately EOI both controllers. Pending slave work
survives either EOI order; a new eligible slave transition can latch IR7 while
the master is still in service, for delivery after its EOI. Regular CPU cascade
acknowledgement still acknowledges both controllers, including existing AEOI
operation.

The original MIT [polling probe](../guest/pic_poll.asm) programs one-shot PIT IRQ0
and FDC READ ID under CLI, checks poll responses and ISR/EOI snapshots, performs
empty final polls, then uses STI to demonstrate ordinary PIT delivery. The guest
checks all 32 result/guard bytes before reporting `PIC POLL PASS`. Native and
production release-Wasm browser tests use the identical image in VM and Ap,
before/after reset, at both browser roots, with unchanged disk bytes and a
2,000,000-slot budget per boot. The [manifest](../fixtures/pic-poll-manifest.json)
pins source/license, assembler command, source/boot/image hashes, exact results
and guards. See [validation evidence](validation.md#issue-63-8259a-ocw3-polling-2026-10-24).

PIC remains **partial**. Polling with AEOI/automatic rotation, concurrent CPU
acknowledgement while a poll is armed, special fully nested mode,
programmable ICW3 routing, MCS-80/85 interrupt output, alternate cascade wiring, edge/level
electrical modeling and spurious-interrupt fidelity remain outside this subset.
These bounded checks do not establish full hardware accuracy or commercial
software compatibility.

### 8253 ordinary counter-latch retention

For counters 0, 1 and 2, an ordinary control word with D5:D4=`00` captures the
current count only when no snapshot is pending. Further latch commands leave
both the snapshot and its next unread byte unchanged, including after the low
byte of a paired read. LSB-only or MSB-only reads release the snapshot after one
byte; paired reads release it after the high byte. A subsequent latch captures
the current running count. Counting, mode, reload, OUT, IRQ progression and other
counters continue independently. Both VM/Ap port banks (`71h..77h` and
`3FD9h..3FDFh`, odd ports) address the same counter state, so commands and reads
can cross banks. Fresh supported-mode programming and Machine reset retain their
existing cancellation behavior.

This is the Issue #55 shared-behavior interpretation of Intel's
[8253/8253-5 Read Operations and Reading While Counting](https://web.cecs.pdx.edu/~mpj/llp/references/8253.pdf)
and the [8254 datasheet, printed pages 7–8](https://www.scs.stanford.edu/23wi-cs212/pintos/specs/8254.pdf#page=7),
which explicitly ignores duplicate ordinary latch commands until the programmed
read completes and identifies the 8254 as an 8253 superset on page 1. It adds no
8254 read-back command. Coverage is bounded to existing binary modes 0/2/3 and
aliases 6/7; unfinished unlatched read insertion and new interleaved count-write
semantics are outside this change.

Original `counter_latch_` [Rust tests](../tests/pit.rs) cover formats, zero,
channels, multiple duplicates, recapture, cross-bank access, count/OUT/IRQ
comparison with a control, fresh modes and reset. The separate original MIT
[guest probe](../guest/pit_counter_latch.asm) exercises both duplicate-command
cases and recapture through guest I/O. Its checked
[manifest](../fixtures/pit-counter-latch-manifest.json) records provenance,
offline assembly, hashes, raw results/guards and a two-million-slot budget.
Native and release-Wasm tests require the same bytes and image before/after
reset on VM/Ap at both browser roots. See the
[validation record](validation.md#issue-55-ordinary-counter-latch-retention-2026-10-24).
PIT remains **partial**; all fixed inventory scopes and deferred work below
remain unchanged.

### 8253 binary mode-4 subset

All three shared counters implement binary software-triggered strobe under the
existing held-high GATE model, through both `71/73/75/77h` and
`3FD9/3FDB/3FDD/3FDFh` aliases, including interleaved cross-bank accesses.
A fresh mode word immediately establishes high OUT and cancels the old count
sequence, pending load, partial write, latch and byte-read phase. Counting waits
for a complete LSB-only, MSB-only or LSB/MSB value; no pre-load count is promised.
Each complete initial or replacement count transfers on the next clock without
decrementing. Binary zero means 65,536. For N=3, successive clock states are
`(3,H), (2,H), (1,H), (0,L), (FFFFh,H), (FFFEh,H)`.

Count-only writes preserve OUT until a clock arrives. The first byte of a pair
leaves the active sequence and any completed pending load intact; the second
schedules the next-clock load. The latest complete value before that clock wins,
even with a following unfinished pair. A low strobe reached before pair completion
still finishes normally. Loading during the low pulse finishes that pulse with
an actual rising edge and restarts the count. Ordinary latch snapshots survive
count-only replacement and wrap until the programmed read completes. After the
single low clock, counting continues through any number of 16-bit wraps with
OUT high and no periodic strobe or IRQ recurrence. Large batches use constant
work and retain any rise crossed inside the batch.

These semantics follow Intel's [8253/8253-5 datasheet, Counter Loading and Mode 4](https://www.cpu-galaxy.at/CPU/Ram%20Rom%20Eprom/Other_Intel_chips/other_intel-Dateien/C8253.pdf).
The [8254 datasheet, Mode 4/Figure 19 and Counter/Figure 22](https://www.scs.stanford.edu/23wi-cs212/pintos/specs/8254.pdf#page=15)
clarifies the shared load clock, paired replacement and continued wrap behavior;
this adds no 8254 read-back command.

Channel 0 actual low-to-high transitions use existing IRQ0/PIC delivery, including
a programming-induced setup rise from prior low OUT. Tests drain that request
separately before measuring the strobe. The falling edge requests nothing;
channel 1/2 transitions cannot request IRQ0. Masked IRR retention, normal
ISR/EOI, IF deferral and natural CPU handler/IRET behavior remain shared.
Channel 1 beeper polarity follows mode-4 OUT at current sample endpoints,
including after wrap. Mode-3 audio is preserved. The existing batch sampler
uses the final OUT for all samples emitted by a bus tick, so short pulses
between endpoints can be missed; no audio scheduler redesign is claimed.

The [Rust tests](../tests/pit.rs) use literal clock traces and independent expected
results. The original MIT [boot probe](../guest/pit_mode4.asm) uses guest port I/O,
cross-bank software replacement and repeated arming, natural IRQ0 handlers,
normal EOI/IRET, advancing post-terminal latches and a quiet interval exceeding
one full wrap. Its [manifest](../fixtures/pit-mode4-manifest.json) pins provenance,
source/boot/image hashes, exact results/guards and the two-million-slot budget.
Hosts only mount, step, inspect and reset. See
[actual validation](validation.md#issue-93-8253-binary-mode-4-2026-10-24).

PIT remains **partial**. Modes 1/5, external GATE/retriggering, BCD, remaining
access/latch sequencing, mode-2/3 divisor-1 behavior and fuller beeper timing
remain outstanding. All 26 fixed inventory scopes/statuses are unchanged;
protected execution, paging, numerical x87 and every other inventory gap remain
required. Limited probes establish neither full hardware accuracy nor commercial
compatibility.

### 8253 binary mode-0 subset

All three counters on VM/Ap and both primary/mirrored timer banks support a
binary mode-0 lifecycle with GATE held high. A fresh mode word sets OUT low,
clears partial read/write/latch state and stops counting until a complete LSB,
MSB or paired initial count is written. Encoded zero means 65,536. The next PIT
clock loads without decrementing; pre-load count readback is undefined.
Each subsequent clock decrements once. The first terminal zero raises OUT, and
the count continues modulo 65,536 with OUT high across later wraps. For count 4,
samples starting at the load clock are `(4,L), (3,L), (2,L), (1,L), (0,H),
(65535,H), (65534,H)`. Ordinary latches preserve a snapshot while counting
continues. A new mode word starts an independent lifecycle, including transitions
between modes 0, 2 and 3. Reset clears all programming and fractional state.

Count-only replacement preserves the programmed mode and access format. A
single-byte write lowers OUT immediately. The first byte of a paired write
lowers OUT, stops counting indefinitely until the second byte, and cancels any
older pending load. Completing a count schedules its transfer on the next PIT
clock, without decrementing on that clock. The latest complete replacement
before that clock wins. Each replacement starts a new terminal-rise lifecycle;
later wraps keep OUT high. Existing completed latch snapshots survive count
replacement, including a snapshot whose low byte has already been read.

This follows Intel's [8253/8253-5 Counter Loading, Mode 0 and Read/Write
Procedures](https://web.cecs.pdx.edu/~mpj/llp/references/8253.pdf).
Immediate first-byte OUT lowering and the separate load clock are explicit in
[Intel 8254 Mode 0, page 10](https://www.scs.stanford.edu/23wi-cs212/pintos/specs/8254.pdf#page=10),
also corroborated by [NEC µPD71054 Table 5](https://www.meditronik.com.pl/doc/plus/upd71054c.pdf).
These support the shared mode-0 timing; no 8254 read-back support is implied.
Only the terminal rise of OUT0 requests IRQ0 through the existing PIC latch.
Falling OUT, held-high OUT and further counter wraps do not request again;
lowering OUT preserves an already latched request, following Intel's
[8259A IRR/ISR and edge-triggered contract](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=17).
Batches report any terminal rise and use constant-time arithmetic, preserving
the existing 1,996,800 / 8,000,000 clock conversion and fractional remainder.

Original [Rust tests](../tests/pit.rs) exercise count/OUT traces, all channels and
access forms, incomplete programming, latches, repeated wraps, mode transitions,
reset, scheduler batching and PIC/CPU delivery. The original MIT
[guest probe](../guest/pit_mode0.asm) checks advancing post-terminal counts and
exactly one hardware IRQ0 per fresh arming, observing another full wrap before
rearming. Its [manifest](../fixtures/pit-mode0-manifest.json) records offline
assembly, hashes, licensing, exact results/guards and a two-million-slot budget.
See [actual validation](validation.md#issue-49-8253-binary-mode-0-2026-10-23).
The separate original MIT [rewrite probe](../guest/pit_mode0_rewrite.asm)
programs mode 0 once, interrupts an active countdown with a delayed pair and
rearms after terminal with count writes alone. It checks no IRQ during either
incomplete pair, exactly one hardware IRQ0 per completed replacement, ISR/EOI,
post-terminal counting and no repeat across a full wrap. Its
[manifest](../fixtures/pit-mode0-rewrite-manifest.json) pins source/image hashes,
license and exact results/guards. See the
[rewrite validation](validation.md#issue-51-mode-0-count-only-rewrites-and-irq0-rearming-2026-10-24).
External GATE, BCD, comprehensive latch/access corner cases and mode-0 audio
remain outside this verified contract.

### 8253 binary mode-2 subset

Both profiles and both timer port banks support mode encodings `010` and `110`
as binary mode 2, with GATE held high. Zero encodes 65,536. Programming this mode
sets OUT high and stops counting until a complete byte sequence is written.
The first subsequent PIT clock loads the count without decrementing it. For
N >= 2, OUT stays high for N−1 clocks, goes low at count 1 for one clock, then
reloads and rises. A completed count rewrite changes only the programmed reload;
the current period finishes before the new divisor loads. Multiple completed
rewrites use the latest value. LSB, MSB and LSB/MSB writes and ordinary count
latches use this state; latching/reading does not stop the counter.

This follows Intel's [8253/8253-5 datasheet, Counter Loading and Mode 2](https://web.cecs.pdx.edu/~mpj/llp/references/8253.pdf).
Channel 0 feeds actual rising OUT transitions into the existing PIC request latch,
including a programming-induced rise from low. A held-high OUT does not retrigger,
as specified for edge triggering in Intel's [8259A datasheet, IR inputs and Edge
and Level Triggered Modes](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=17).
A batch reports whether any rise occurred, including a complete low pulse between
high endpoints, and leaves the same count/OUT and fractional clock remainder as
single-clock advancement. Whole periods are skipped with bounded arithmetic;
multiple elapsed rises latch one request without an interrupt backlog. Clock
conversion remains 1,996,800 Hz from the 8 MHz guest scheduler on both profiles.

Original [PIT tests](../tests/pit.rs) provide independent count/OUT traces,
rewrite/read/batch checks and PIC/CPU integration. The separate MIT
[boot probe](../guest/pit_mode2.asm) verifies deferred reload with guest counter
reads and services two periodic IRQ0 entries with EOI/IRET. Its
[manifest](../fixtures/pit-mode2-manifest.json) records source/image hashes,
result/guard layout, offline preparation and the two-million-slot boot budget.
See [actual validation evidence](validation.md#issue-45-8253-binary-mode-2-2026-10-23).

### 8253 binary mode-3 subset

All three counters, both profiles and both timer port banks support encodings
`011` and `111` as binary mode 3 with GATE held high, for divisors 2–65,536.
Encoded zero represents 65,536. A new mode word sets OUT high, clears partial
read/write/latch state and waits for a complete initial LSB, MSB or paired count.
The next PIT clock loads the divisor without decrementing. Pre-load count values
are undefined and are not part of the tested contract.

Intel's [8253/8253-5 datasheet, Control Word Format, Counter Loading, Mode 3 and
Read/Write Procedures](https://web.cecs.pdx.edu/~mpj/llp/references/8253.pdf)
specifies decrement-by-two for even divisors. An odd divisor first subtracts one
in the high half and three in the low half, then subtracts two on later clocks.
At terminal count the full divisor reloads and OUT toggles, giving high and low
durations of ceil(N/2) and floor(N/2) clocks. Starting at the load clock:

| Divisor | Successive (count, OUT) samples |
| --- | --- |
| 4 | (4,H), (2,H), (4,L), (2,L), (4,H) |
| 5 | (5,H), (4,H), (2,H), (5,L), (2,L), (5,H) |

A completed count-only replacement preserves the active count and OUT sequence
until the current half-cycle's terminal boundary. OUT toggles at that boundary
and the latest completed divisor loads for the next half-cycle, without an extra
initial-load clock. Paired writes complete only on the MSB; an unfinished pair
does not stop counting or replace the last completed divisor. Access format and
ordinary completed latch snapshots remain intact. Fresh mode words and Machine
reset cancel deferred replacements. This reload timing follows the explicit
half-cycle replacement rule in Intel's [8254 datasheet, Mode 3, printed pp.
12–13](https://www.scs.stanford.edu/23wi-cs212/pintos/specs/8254.pdf#page=12), while
retaining the 8253 odd-count readback above rather than 8254 readback semantics.

For example, load 8 with control `16h`, advance two decrement clocks to `(4,H)`,
then write 6 without a mode word. The next clocks are `(2,H)`, `(6,L)`, `(4,L)`,
`(2,L)`, `(6,H)`; only the last transition requests IRQ0. Independently, load 7,
advance four high clocks and one low decrement to `(4,L)`, then replace with 4:
`(2,L)`, `(4,H)`, `(2,H)`, `(4,L)`, `(2,L)`, `(4,H)`.
Run `cargo test --locked --test pit mode3_rewrite_` after preparing the probe.

Ordinary reads and count latches expose this sequence while counting continues.
OUT0 rising transitions, including mode-programming rises, feed the existing PIC
latch as in mode 2. Falling transitions and held-high output do not request IRQ0,
following Intel's [8259A edge-triggered IR-input contract](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=17).
Bounded arithmetic skips entire periods and reports any intervening rise even
when both batch endpoints are high or both are low. Final count, OUT, phase and
fractional scheduler state match single-clock advancement. Multiple rises still
coalesce into one PIC request. Scheduler conversion is unchanged.

The mode-3 beeper uses channel 1 OUT for polarity, retaining the existing enable,
amplitude, 48 kHz sampling and 4096-sample queue bound. Other modes retain their
previous mixer behavior. Audio resampling within large scheduler batches is
unchanged. New mode words support reprogramming and transitions between modes
2 and 3; reset clears active, partial, latched and fractional state.

Original [Rust tests](../tests/pit.rs) cover specification traces and boundaries,
all channels/access forms, latches, batching, PIC/CPU integration and reset.
The separate MIT [boot probe](../guest/pit_mode3.asm) proves decrement-by-two
through guest latches and services periodic hardware IRQ0 with EOI/IRET. Its
[manifest](../fixtures/pit-mode3-manifest.json) records offline assembly, hashes,
provenance, exact results/guards and the two-million-slot budget. See the
[validation record](validation.md#issue-47-8253-binary-mode-3-2026-10-23).

The separate MIT [count-only rewrite probe](../guest/pit_mode3_rewrite.asm)
rejects premature restart using count latches and checks natural IRQ0 service
with ISR/EOI/IRET. Its [manifest](../fixtures/pit-mode3-rewrite-manifest.json) pins
offline source, boot and image hashes, results/guards and the same per-boot budget.
See [rewrite validation](validation.md#issue-53-mode-3-count-only-rewrites-at-half-cycle-boundaries-2026-10-24).

PIT remains **partial**. Mode-specific behavior for 1/5,
external GATE/retriggering, BCD, divisor-1 behavior, comprehensive latch/access
sequencing and fuller beeper timing remain outstanding. These shared counter
subsets do not establish instruction-cycle accuracy, full audio or hardware
accuracy, or commercial compatibility. All 26 fixed inventory categories, scopes
and statuses are unchanged; the existing six-game reports retain their limits.
Protected execution, paging and basic FPU remain required project goals.

The hello source uses `OUT 3FDFh,AL`, an immediate-port encoding that truncates to
`DFh`. That timer mirror is decoded too. PIT behavior beyond the subset above,
DMA arbitration and request modes, precise FDC motor/rotation timing, FDC MT/head
switching, formatter/scanner commands, GDC vector/area drawing and all GDC
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

The DMA FDC backend samples eligibility at its scheduled service boundary,
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

## FDC non-DMA READ DATA

On VM and Ap, SPECIFY (`03h`) retains ND (last parameter bit 0). With ND set,
`46h` supports single-head MFM READ DATA (MT=SK=0), N=1–3, and ascending sectors
through EOT on existing raw/D88 media. Select `0094h=18h`. Other ND read variants
return invalid-command status; the bounded PIO write subset is described below. Clearing ND
restores the existing DMA reads/writes.

Execution holds CB/EXM/DIO; RQM rises for one available byte. Reading `0092h`
consumes that byte and lowers its request; MSR reads and wrong-direction writes
preserve it. Result phase clears EXM, and all seven bytes must be drained before
CB clears. Data and result requests use slave IR3/master IR7; normal PIC EOIs
remain necessary. The first result read acknowledges the result request. This
follows [NEC µPD765 processor interface and READ DATA](https://www.scribd.com/document/1067140199/UPD765)
and [µPD765A/B Tables 2–4](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/UPD765_Datasheet_OCRed.pdf).

The deterministic approximation keeps the existing 160-clock command dispatch.
The first byte follows 128 clocks later; subsequent arrivals remain 128 clocks
apart, including across sector boundaries, independently of read time. A byte
must be serviced before 104 further clocks expire. At 8 MHz these are the normal
16 µs MFM byte interval and worst-case 13 µs service interval described in
[NEC Application Note 8, PDF page 2](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/uPD765_App_Note_Mar79.pdf).
Equal deadlines expire before a subsequent port read. Port accesses do not
advance time. Tick overshoot crosses every event, including availability followed
by timeout in the same tick. After the final consumed byte, the next byte boundary
starts results. No DMA registers, shared pointer or TC state participate.

An unserviced byte ends with abnormal ST0 and ST1.OR (`10h`); unavailable sectors
use ST1.ND (`04h`). EOT without TC ends with abnormal ST0 (`40h | head<<2 | drive`),
ST1.EN=`80h`, ST2=0 and the final sector's C/H/R/N. This derives from the EOT
stop condition, EN definition and sector-ID result fields. NEC READ DATA Table 2
explicitly describes **processor termination**; its C+1/R=1 EOT row is not used
for this controller-terminated path. Existing successful DMA TC results remain
unchanged. The probe pins `40 80 00 00 00 03 03` for sectors 2–3, head 0, N=3.

[Issue #79 validation](validation.md#issue-79-fdc-non-dma-read-data) covers raw-port
transitions and an original MIT interrupt-driven boot probe. FDC remains partial:
MT/head switching, N=0/DTL, deleted-data/format/scan/diagnostic commands,
full SPECIFY timing, TC/EOP injection, mechanical/flux timing, new formats, broad
error fidelity and comprehensive external board controls remain outside this
subset. Protected execution, paging and basic x87 remain required project work;
these probes make no complete hardware or commercial compatibility claim.

## FDC non-DMA WRITE DATA

On VM and Ap, ND=1 also accepts single-head multi-sector MFM WRITE DATA
(`45h`, MT=0), matching selected head/H, N=1–3 and 1≤R≤EOT. Both drives and
either head use the existing normal raw/D88 sector lookup. Each sector transfers
`128 << N` bytes; DTL does not shorten it. After each sector, R increments while
C/H/N remain fixed, without an intermediate result or another command. A missing
later C/H/R/N stops at that requested ID with ST1.ND; completed payload remains
and later sectors and all metadata remain unchanged. Other write variants return
one-byte invalid-command `80h` without DMA or media activity. ND=0 retains the
existing DMA backend.

During dispatch and execution MSR is `30h` while waiting, `B0h` when accepting
one byte (CB/EXM set, DIO clear). Only a ready FIFO write consumes a request.
Early/duplicate writes and wrong-direction reads leave the transfer untouched;
port accesses do not create elapsed time. All seven results expose `D0h`, then
idle is `80h`. Results must be drained before another command is accepted.
The [NEC functional description](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/upd765A.pdf)
specifies bytewise writes and applies READ DATA's EN behavior to WRITE DATA.
The [NEC status/instruction tables](https://hxc2001.com/download/datasheet/floppy/thirdparty/FDC/NEC/UPD765_Datasheet_OCRed.pdf#page=9)
define direction, non-DMA execution, write protection and error flags.

Writes share the read scheduler: 160 clocks to dispatch, first request 128 clocks
later, subsequent requests every 128 clocks including sector boundaries, and
timeout at 104 unserviced clocks. Service at 103 clocks succeeds without moving
the next boundary; service at the 104-clock deadline is too late. These are deterministic emulator choices derived
from NEC's MFM intervals at the repository's 8 MHz clock, not mechanical or
instruction-cycle accuracy. Every byte request interrupts via slave IR3/master
IR7, its servicing write acknowledges the FDC request, and completion generates
a distinct result request. PIC ISRs still require normal EOIs.

Without TC, R=EOT ends with `40h | head<<2 | drive`, ST1.EN=`80h`, ST2=0 and
final C/H/R/N, using the [read derivation above](#fdc-non-dma-read-data).
This final-ID policy is the repository's interpretation of NEC's EOT/EN and
result-field descriptions, without independently measured hardware evidence.
Protected media reports ST1.NW=`02h`, absent/missing sectors ST1.ND=`04h`, and
first/interior missed service ST1.OR=`10h`, with abnormal ST0. No DMA address,
count, bank, mode, mask, controller enable, shared byte pointer or sticky TC is
consumed by PIO.

Each accepted byte immediately changes only its logical sector payload byte.
Timeout, reset or eject retains the accepted prefix and leaves the unaccepted
suffix, neighboring sectors and all D88 metadata unchanged. One sector receiving
any payload increments the FDC/media write counters once. This is an image-model
approximation: damaged data fields, CRC and flux consequences are not represented.
No TC-specific zero filling applies to commands that receive no TC. Reset
cancels execution and requests; eject or replacement on the active drive ends
execution with missing-data results before another byte can reach the new media.

The original MIT [interrupt-driven boot probe](../guest/fdc_write.asm) supplies
1,024 bytes, checks PIO readback, both seven-byte results, real PIC ISR/EOI
records, guards and SP, then prints `FDC WRITE PASS`. Its
[manifest](../fixtures/fdc-write-manifest.json) pins source/license, assembly,
firmware/image hashes, independent expected media/results and a two-million-slot
budget. Native and release-Wasm VM/Ap run the identical image before/after reset,
with browser paths `/` and `/pc98-rust/`. See
[Issue #83 validation](validation.md#issue-83-fdc-non-dma-write-data).

The separate original MIT [multi-sector probe](../guest/fdc_write_multi.asm)
issues one R=2/EOT=3 write and one readback command. Guest handlers service
exactly 2,048 byte interrupts per direction and one result interrupt per command,
with both PIC EOIs. It checks distinct sector payloads, `40 80 00 00 00 03 03`
results, guards and restored SP before printing `FDC WRITE MULTI PASS`. Its
[manifest](../fixtures/fdc-write-multi-manifest.json) pins source/license, assembly,
firmware/image hashes, independent result/media oracles and the same
2,000,000-slot budget. Native and production release-Wasm VM/Ap run this identical
probe before/after reset at both browser paths. See
[Issue #85 validation](validation.md#issue-85-single-head-multi-sector-fdc-non-dma-write-data).

MT/head switching, deleted-data/format/scan/diagnostic commands, N=0/DTL, new
formats, full SPECIFY timing, TC/EOP injection, active BIOS preemption and broader board/mechanical/error fidelity remain out of scope.
Additional media and 512-byte IPL acceptance remain FDC work. The A/B final-byte
OR distinction is unverified; missed-service tests cover first/interior requests.
FDC remains partial; all 26 inventory statuses and full scopes remain unchanged.
Protected mode, paging and basic x87 remain required.

## BIOS floppy DMA handoff

From an idle FDC with retained ND=1, the synthetic BIOS's floppy transfer helper
sends SPECIFY `03 DF 02` through port `0092h` before seeking or submitting
READ/WRITE DATA. It checks the retained controller mode to avoid a redundant
command and preserve existing request timing when DMA is already selected.
This selects SRT=13, HUT=15, HLT=1 and ND=0. Timing fields remain unmodeled; these
are synthetic firmware parameters, not a claim about proprietary NEC BIOS
internals. SPECIFY has no execution/result phase. The helper waits for idle
command readiness (`MSR=80h`) with a 160-clock dispatch budget before proceeding.
Transfers still use the existing channel-2 DMA, controller command/result FIFO,
scheduler and interrupt path. No sector payload bypasses the controller.

An earlier raw client may leave ND=1; BIOS selects DMA for its own request and
does not restore ND. A subsequent fresh SPECIFY ND=1 still enables supported
byte-handshaked single-head PIO reads and writes. The original source-built probe
and native regressions cover reads/writes on VM/Ap, same-cylinder/seek-required
requests, error recovery, complete media differences, payload/guard checks, DMA progression/TC, drained
results and PIO reentry. See [Issue #81 validation](validation.md#issue-81-bios-floppy-dma-handoff).
Active-command preemption, BIOS VERIFY redesign and broader FDC functionality
remain outside this idle-controller handoff contract. The FDC inventory remains
partial; this probe does not establish full hardware accuracy or commercial
software compatibility.

## Ap IDE READ VERIFY

The existing bank-zero master accepts both `40h` and `41h` in CHS and LBA modes.
Input count zero means 256 sectors; other values request 1–255. Successful
completion leaves count zero and the address of the last verified sector.
An invalid starting address returns IDNF (`10h`) with the original address/count.
A request crossing capacity completes its valid prefix, then returns the first
failing address and the remaining count including that sector. Residual 256 is
encoded as zero. Device/head control bits are preserved. Completion has BSY and
DRQ clear, ERR clear on success and set on failure, and the usual completion IRQ.

This is the explicit legacy task-file contract from
[Fujitsu's READ VERIFY description, printed page 5-22](https://www.manualslib.com/manual/54913/Fujitsu-Mhc2032at.html?page=93),
selected for this emulator. The later
[T13/1321D revision 3, section 8.28](https://www.seagate.com/support/disc/manuals/ata/d1153r17.pdf#page=169)
defines the non-data command, count-zero convention and IDNF behavior but leaves
normal address/count outputs unspecified. Last-sector success outputs are
therefore not presented as a universal ATA requirement.

VERIFY completes synchronously with bounded arithmetic over the HDI geometry.
It never loads a PIO buffer, transfers host data, asserts DRQ or writes media.
Validated HDIs have no modeled ECC failures; this checks addressability and does
not claim physical-media integrity checking. Alternate-status reads preserve the
device IRQ latch; primary-status reads clear it. nIEN suppresses delivery while
retaining the latch. An already latched slave PIC IR1 request survives device
acknowledgement and requires its own PIC acknowledgement/EOI.

The Ap IDE wire latches slave IR1 only on inactive-to-active enabled INTRQ.
Its input history is independent of PIC IRR/ISR and survives PIC initialization:
acknowledgement, EOI and initialization cannot turn a held input into a new edge.
Machine reset recreates both the device and wire history coherently. Port side
effects are sampled immediately, including primary status, nIEN/SRST and PIO
sector boundaries, so consecutive operations between clock ticks retain edges.
SRST clears pending state; nIEN retains it, and re-enabling a pending output forms
a genuine edge. This follows
[Intel 8259A, initialization and edge-triggered modes, printed pp. 9 and 17](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf)
and [T13/1321D revision 3, §§6.3, 7.3, 7.9 and 7.15](https://www.seagate.com/support/disc/manuals/ata/d1153r17.pdf)
(the linked file is an ATA/ATAPI-5 working draft despite its filename).

Original [Rust tests](../tests/hdd.rs) cover both opcodes/modes, counts 1/2/255/256,
CHS transitions, LBA carry, final-sector success, invalid addresses, inclusive
residuals, recovery, IRQ/nIEN, byte-identical media and subsequent IDENTIFY/read/write.
The MIT [guest probe](../guest/ide_verify.asm) independently checks 46 literal
result records, bounded held-input silence during service and after normal EOIs,
IRQ observations, two actual INT 11h entries through the cascade, recovery and
guards before PASS. Its
[manifest](../fixtures/ide-verify-manifest.json) pins offline assembly, source,
boot/image/synthetic-firmware hashes, geometry, exact oracle bytes and a
500,000-slot budget. Native and production release-Wasm tests boot the identical
HDI before/after reset at both browser roots; see
[validation evidence](validation.md#issue-77-ap-ide-irq-edges-2026-10-24).

IDE remains **partial**. Completed 256-sector PIO transfers, READ/WRITE final
addresses and error residuals, and fuller reset, nIEN and device/error interactions
remain outstanding. No DMA/ATAPI/SCSI/SASI, defective-sector infrastructure,
mechanical timing or BIOS redesign is added. All 26 inventory categories, scopes
and statuses remain unchanged. Protected execution, paging and basic FPU remain
required. Existing limited probes and game reports do not establish complete
hardware accuracy or commercial compatibility.

## OPN/OPNA channel-3 frequency selection

The shared `Opn` core selects the following **bank-zero** pairs when register
27h D7:D6 is `01` (40h) or `10` (80h). With `00`, all four operators use A2h/A6h.
These are logical operators; ordinary operator parameter registers still use
physical order 1,3,2,4.

| Logical operator | F-number low | Block/F-number high |
| --- | --- | --- |
| 1 | A9h | ADh |
| 2 | AAh | AEh |
| 3 | A8h | ACh |
| 4 | A2h | A6h |

F-number is 11 bits (high register D2:D0 plus low byte); block is high-register
D5:D3. Write high before low. The existing default clock is 3,993,600 Hz, with
frequency `FNUM * 2^BLOCK * clock / (144 * 2^20)`. Normal mode ignores extra
pairs. Mode changes retain programmed frequencies and active operator phase,
envelope and key state. Multiplication, approximate detune/envelopes and algorithm
routing are preserved. Other channels retain their ordinary pairs. Upper-bank
extra pairs and mode writes cannot alias bank zero. Existing Ap board masks and
register-29 channel enable remain in force; VM has no exposed sound board.

References: Yamaha [YM2608 Application Manual §§2-2-2, 2-4-1 and 2-8-4](https://csclub.uwaterloo.ca/~pbarfuss/YM2608J_Translated.PDF#page=20)
and [YM2203 datasheet register descriptions (10), (19), (20)](https://bitsavers.org/components/yamaha/YM2203_198911.pdf#page=5).

The original MIT [assembly probe](../guest/opn_ch3.asm) and
[manifest](../fixtures/opn-ch3-manifest.json) provide 16 stable PCM stages, four
per keyed operator: normal → 40h → 80h → normal, with both timers stopped.
Native and production release-Wasm hosts capture PCM through the existing audio
queue every 1,000 slots and quantitatively check pitch. Markers identify stages;
they do not stand in for audio assertions. Each boot has a 2,000,000-slot limit,
exact result/guard checks and unchanged disk data, before/after reset and at both
browser roots. See [validation evidence](validation.md#issue-67-opnopna-channel-3-frequency-selection-2026-10-24).

OPN and OPNA remain **partial**. This implements only their common channel-3
frequency selection, not either complete named special mode. CSM timer-driven
keying is absent. Combined encoding `11` is explicitly unverified and currently
follows ordinary frequency selection; that fallback is not a hardware claim.
Frequency-write latch timing, prescalers, key scaling, fuller envelopes/SSG-EG
and broader synthesis verification remain outstanding. OPNA also retains LFO,
pan/stereo, rhythm and ADPCM gaps. VM sound exposure remains outstanding.
No inventory scope or status is promoted by these bounded probes.

## Ap YM2608 timer IRQ enables

Ap bank-zero register 29h D0/D1 independently enable Timer A/B IRQ output.
D7 (SCH) retains its existing FM4–FM6 channel-enable role. Construction and
Machine reset set 29h to `1Fh`: all five enables set, SCH clear, reserved D6:D5
zero. The existing register storage retains written reserved bits, with no
behavior assigned to them. D4:D2 are retained enables for unimplemented sources;
no ADPCM-related flags are fabricated.

For the currently implemented sources, device output is
`(timer_flags & register_29 & 03h) != 0`, evaluated on each IRQ query. Changing
29h immediately exposes or suppresses already-pending flags. It does not clear
status, reload timers or discard accumulated/fractional clocks. Register 27h
continues to control loading, overflow flag generation and independent flag
clearing. Status reads are nondestructive. Timer frequencies are unchanged.

The bus ORs OPNA output with PCM's independently enabled FIFO IRQ at slave IR4:
`opn.irq() || (pcm.irq && pcm.control & 0x20 != 0)`. One logical edge detector
observes this combined wire after relevant guest writes, timer advancement and
PCM audio sampling. Only inactive-to-active transitions latch IRR4; holding the
wire active across CPU or poll acknowledgement and normal EOI cannot request it
again. Consecutive output-disable/enable writes remain visible without a tick.
Adding or clearing one contributor while the other holds the wire active does
not create an edge. A PCM threshold crossing is observed in the same bus tick
as FIFO consumption, even when no further scheduler tick occurs.

Suppression changes only device output: latched PIC IRR and ISR survive until
ordinary acknowledgement and EOI. Acknowledgement clears IRR4 and sets ISR4;
a fresh edge during service remains pending for later delivery. Masked edges
remain captured for unmasking. Both cascade controllers require normal EOI.
PIC initialization clears its request/service registers but retains the bus's
input history, so an already asserted input cannot fabricate an edge. Machine
reset reconstructs the devices and their edge history for fresh delivery.
Existing board I/O masks and upper-bank isolation remain in force. The probe explicitly programs slave base
18h to observe vector 1Ch; it does not change the emulator's existing PIC reset
or synthetic firmware base 10h. The shared Rust name `Opn` does not imply that
YM2203 has register 29h; VM sound remains unexposed.

References: Yamaha [YM2608 Application Manual §2-2-1, page 19](https://csclub.uwaterloo.ca/~pbarfuss/YM2608J_Translated.PDF#page=19)
and [§2-8-3, page 36](https://csclub.uwaterloo.ca/~pbarfuss/YM2608J_Translated.PDF#page=36),
and Intel [8259A interrupt sequence and EOI](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=7),
[initialization](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=9)
and [edge-triggered input](https://courses.cs.washington.edu/courses/cse451/25wi/documentation/8259A.pdf#page=17).

The original MIT [guest probe](../guest/opna_irq.asm) and
[provenance manifest](../fixtures/opna-irq-manifest.json) cover both timers,
suppressed overflow/status reads, enabling pending flags, retained requests,
actual guest handler execution, ISR observations, EOIs and subsequent recovery.
The enabled recovery keeps device output asserted across both ordinary EOIs
and a bounded 128-iteration guest delay. All eight master/slave poll bytes must
be zero; the guest checks each pair before suppressing/clearing the source.
There is no extra cleanup EOI pair. These observations are checked separately
from the exact four CPU handler entries. Every result
and guard byte is checked by guest and host, before/after reset, natively and in
production release-Wasm at `/` and `/pc98-rust/`, within 500,000 slots per boot.

OPN and OPNA remain **partial**. LFO, pan/stereo, rhythm, ADPCM playback/registers/
IRQs, absent proprietary rhythm samples and broader synthesis validation remain
outstanding. Shared gaps include CSM keying, combined channel-3 mode, frequency
write latching, prescalers and fuller envelopes. Protected execution, paging and
basic x87 remain required elsewhere. All 26 inventory scopes and statuses remain
unchanged; this bounded timer contract establishes no complete OPNA accuracy or
commercial compatibility.

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
| INT 1C AH=0,2 | VM fixed BCD date; Ap [guest-clocked calendar snapshot](#ap-calendar-read-path); separate one-shot interval callback (see below) |
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
