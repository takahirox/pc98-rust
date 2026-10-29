# Ap protected mode and i486 paging (Issues #101, #103, #107)

PC-9821Ap DOS now executes an original-Rust ring-0 protected subset with
CR0.PE=1, with paging disabled or the [4 KiB paging subsystem](#i486-4-kib-paging-issue-107) enabled. The default VM/V30 decoder and devices retain their
existing behavior. This is bounded CPU capability evidence, not a claim that a
DOS extender, operating system or commercial game works.

The contracts were independently checked against Intel's
[80386 PRM](https://pdos.csail.mit.edu/6.828/2018/readings/i386.pdf), chapters
6 (descriptors/protection), 9 (interrupts), 14 (real-mode return), 16 (sizes),
17 (instruction operations), and the 1992 Intel486 PRM CR0 contract cited in
[hardware.md](hardware.md#ap-real-mode-control-register-writes). Intel also
publishes its [architecture manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).
No other emulator source, translated core, emulation library, proprietary ROM,
font dump or guest host-execution mechanism was used. Cargo dependencies are unchanged.

## Implemented contract

- **CR0:** a valid MOV CR0 PE enable commits, with ET fixed at 1. CR0 readback,
  SMSW and existing x87 EM/TS checks share that state. CR3 selects physical
  paging tables; CR2 records the last faulting linear address. LMSW changes low control bits but cannot clear
  an already set PE. PG without PE and NW without CD raise #GP before mutation.
  Valid PE=1/PG=1 enables translation; PG toggles flush the TLB and retain
  segment caches. Reserved writes retain their atomic undefined-bit diagnostic.
- **Transition window:** PE changes protection checks without replacing hidden
  CS/SS/data attributes. A far transfer reloads CS from the descriptor table.
  Clearing PE retains caches until segment reloads; a real data load changes
  the base to selector × 16, retaining the cached limit/B. A real far code
  transfer restores conventional code access/default size and replaces the base,
  retaining the limit. The accepted return first loads 16-bit, byte-granular,
  writable 64 KiB stack/data and 16-bit 64 KiB code, disables interrupts, clears
  PE, far-jumps to real code, reloads IDTR and then reloads real data segments.
  No guest PE bit is silently cleared by the emulator. Reset clears caches,
  LDTR, TR and fault metadata and restores real-mode control/table reset state.
- **Segments:** visible selectors and hidden base, inclusive limit, type/access,
  presence and default-size attributes for CS/SS/DS/ES/FS/GS. GDT and LLDT-loaded
  LDT fetches use complete eight-byte bounds checks, including LDT entry zero.
  SLDT reads the selector; LLDT caches the table independently of later GDT edits.
  Successful code/data loads set the descriptor accessed bit through the bus.
  Null data selectors are unusable; null CS/SS, invalid type, missing tables,
  DPL/RPL violations and nonpresent descriptors raise #GP/#NP/#SS with selector
  error codes. Writable/readable/conforming code, expand-up and expand-down data,
  byte/page granularity and complete operand spans are checked before writes.
- **Addresses and sizes:** CS.D selects default operand/address size; 66/67
  override independently, and repeated prefixes do not toggle repeatedly.
  EIP and effective offsets may exceed 64 KiB. SS.B independently chooses SP or
  ESP, including pushes/pops, ENTER/LEAVE, interrupt frames and POP [ESP].
  ENTER selects and wraps its BP/EBP frame-chain reads by operand size,
  independently of SS.B and the address-size override (Intel PRM, page 295).
  Near/far JMP/CALL/RET, segment MOV/POP, LES/LDS/LSS/LFS/LGS and IRET/IRETD use
  cached attributes and validate destination descriptors before committing.
  Linear base+offset wraps at 32 bits and, after paging when enabled, reaches
  the existing PC-98 physical bus, including 4 MiB RAM, device windows, ROM write protection and
  the F2/F6 A20 latch. No IBM AT/VGA addressing or port assumptions are added.
- **Interrupts and faults:** same-ring 286/386 interrupt and trap gates, with
  gate-selected word/dword EIP/CS/(E)FLAGS frames independent of CS.D and SS.B.
  INT/INT3/INTO use IDT gates with software DPL checks; hardware IRQs use the real
  PIC/PIT routing and ignore software gate DPL. Interrupt gates clear IF; trap
  gates preserve it. Both clear TF/NT/RF after saving flags. Faults save the
  first-prefix EIP, RF and the architectural error-code slot where applicable;
  selector errors include IDT/TI and external-event bits. IRET validates return
  code/offset and restores flags, including RF for a protected IRETD.
  Descriptor loads, multi-push instructions and whole interrupt frames are
  preflighted; rejected operations do not leave partial RAM/device writes.
  Contributory faults during contributory-fault delivery become #DF(0).
  Failed #DF delivery terminates with a triple-fault/shutdown diagnostic.
  Other nested-delivery chains have an explicit three-attempt shutdown bound.
- **Trace:** opt-in headless output remains bounded to 100 instruction records
  and 16 retained control-register events, including their next attempted
  instruction. It now fetches opcodes through cached CS, uses CS.D for displayed
  sizes, and reports mode, CPL, caches/selectors, CR0, GDTR/IDTR, LDTR and the most
  recent fault vector/error code. Issue #103 adds named STR/LTR records, visible
  TR and cached TSS metadata, plus ordinary-RAM descriptor metadata for a register
  LTR source. STR/LTR share the bounded control history and next-attempt capture.
  Only opcode/ModR/M and CPU metadata are read;
  no surrounding guest code or operand memory is dumped.

## i486 4 KiB paging (Issue #107)

The original [paging module](../src/cpu/paging.rs) implements two-level 32-bit
translation after segmentation. Its independent sources are the
[1992 Intel486 PRM](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143),
§§5.3, 6.8, 9.9.8/14, 4.1.3, and the INVLPG entry (26-172), plus Intel's
[80386 paging description](https://pdos.csail.mit.edu/6.828/2018/readings/i386/s05_02.htm)
and [exception description](https://pdos.csail.mit.edu/6.828/2018/readings/i386/s09_08.htm).
The 486 WP and PCD/PWT rules take precedence over the older 386 contract.

| Feature | Implemented behavior / explicit boundary |
| --- | --- |
| Address assembly | CR3[31:12] physical directory; linear[31:22] PDE, [21:12] PTE, [11:0] offset; 32-bit linear wrap; every affected page checked |
| CR0/CR3 | Existing reserved/fixed-bit and PG-requires-PE/NW-requires-CD checks; PG enable/disable retains segment caches and flushes TLB; every MOV CR3 flushes even on same-value reload; PCD/PWT retained without data-cache claims |
| CPU references | Instruction bytes, all shared operand/stack/RMW/string helpers, GDT/LDT/IDT reads, descriptor accessed/busy updates and interrupt/fault frames translate; system-table references use supervisor permission independently of CPL |
| Permissions | Present at both levels; RW and US combine across both levels; CPL 0–2 are supervisor, CPL 3 is user; WP=1 enforces supervisor read-only protection, WP=0 permits supervisor writes; full nonzero-CPL execution remains unsupported |
| A/D | Physical A updates on walks, including a present PDE before an absent PTE; both PDE A and PTE A are set on page-protection faults (Intel486 PRM §9.9.14); D before an actual write, after full operand preflight, stays clear on rejected writes; no PDE D updates; software flags preserved |
| Faults | #PF(14), CR2 = exact first inaccessible linear byte; only P/W/U error bits, including instruction reads (no modern instruction-fetch/NX or RSVD bit); protected 16/32-bit frame and first-prefix IRET/IRETD retry |
| Nested delivery | Contributory→PF remains serial; PF→contributory/PF and contributory→contributory become #DF(0); failed #DF delivery shuts down; other chains retain a three-attempt bound; a fault fetching handler instructions is a new instruction fault |
| TLB | Original 32-slot direct-mapped cache of frame, combined permissions, PTE address and D state; no cached nonpresent entries; guest table edits do not invalidate it; replacement is deterministic emulator policy, not exact 486 associativity/cycles |
| INVLPG | `0F 01 /7` calculates the overridden segment/effective address and invalidates that linear page; no operand access or page walk; register/LOCK #UD, nonzero protected CPL #GP; no-op if no entry or PG=0 |
| Physical bus | Page walks bypass CPU translation; final frames and all walk cycles retain PC-98 A20 decoding; RAM, text/graphics, ROM write protection and unmapped reads retain existing bus behavior; actual FDC DMA reads/writes remain physical |
| Reserved entries | Nonpresent entries are wholly software-owned. Present bits 7/8, reserved in the selected 486 contract, stop with a bounded diagnostic; no PSE/PAT/global semantics or fabricated reserved-bit #PF. PDE bit 6 is undefined and preserved; software bits 9–11 are preserved |
| Table storage boundary | Directory/table words must resolve to ordinary writable physical RAM. ROM, graphics/MMIO or absent table storage stops before a walk read or A/D mutation at that location. The physical PC-98 bus exposes no page-fault/bus-error signal for such storage; no synthetic guest #PF is invented |
| Out of scope | PSE/PAE/NX/global pages, privilege changes, TSS stack/I/O bitmap and task switching, VM86, DPMI, DOS extenders, Windows, numeric x87, data-cache cycles/external cache signals. INVD/WBINVD retain unsupported-instruction diagnostics |

Paged instructions and each exception-delivery attempt journal memory changes.
On rejection, CPU registers/TLB return to the instruction snapshot, RAM/graphics
stores and EGC source/pattern latches roll back, and PEGC bank latches restore.
Successful operations preserve the existing bus read/write order. Observed page
Accessed bits may remain set on a fault; rejected stores do not leave Dirty bits
or partial operand writes. REP still commits one element per step, preserving
completed elements and restarting only the faulting element. String input
preflights its destination before consuming a port value. No device scheduler or
DMA transfer runs inside a CPU instruction's journal.

Clearing guest A/D bits does not clear the corresponding cached state. Software
must invalidate before expecting new accounting. A cached clean mapping's first
write sets D in its remembered physical PTE; later writes do not restore a D bit
cleared without invalidation. Page permissions, including changed CR0.WP, are
checked on every TLB hit. Fault rollback may evict newly filled entries; no
architectural promise is made about TLB retention/replacement across faults.
The bounded trace uses a read-only translation lookup, without A/D/TLB changes
or graphics latch reads, so enabling diagnostics does not change guest paging.

The MIT [boot guest](../guest/paging.asm) enables PG with identity transition
code, then jumps to nonidentity code at linear 00400000h (physical 00100000h).
Its data and stack also use different frames. A guest #PF handler validates
error/CR2/CS:EIP/RF, repairs a missing PTE, invalidates, and IRETD retries the
original store. It checks stale mappings, INVLPG, CR3 reload, 16-bit code with
66/67, stack/data guards and PG disable before printing `PAGING PASS`.
The [manifest](../fixtures/paging-manifest.json) pins source, boot, disk, license,
exact 64-byte results and the slot bound. Browser JavaScript only mounts, steps
the production Rust/Wasm core, inspects results and resets.

```sh
python3 tools/prepare_paging_fixture.py
cargo test --locked --lib --test paging --test dma
cargo run --locked --bin headless -- fixtures/paging/paging.hdm 50000 --ap
# After every README fixture producer, run the full native and browser checks:
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npm test
```

[Validation and baseline reproduction](paging-validation.md) record the actual
commands/results, serving-port override and private-HDD nonvalidation. Existing
independently reproduced task/privilege and numeric-FPU boundaries remain grouped
follow-ups in this document's roadmap. No subsequent private-HDD blocker was
observed or inferred, and no Windows or game-playability claim is made.

## Task-register initialization (Issue #103)

Ap implements `0F 00 /1 STR r/m16` and `/3 LTR r/m16` in protected mode.
Both use **two-byte operands regardless of CS.D or 66**; register STR preserves
bits 31:16, and LTR ignores source bits 31:16. The 67 prefix selects effective
address size independently, including SIB and full offsets; segment overrides
use the existing caches and complete word-span/access checks. LOCK raises #UD
before any operand read/write; group-0 /6 and /7 are invalid and raise #UD.
Real-mode STR/LTR raise #UD. VM keeps its separate V30 `0F 00` interpretation.
Flags and unrelated registers/memory are preserved.

The independent specifications are the
[1992 Intel486 PRM](https://bitsavers.trailing-edge.com/components/intel/80486/1992_Intel_486_Programmers_Reference_Manual.pdf),
printed pp. **26-209 (LTR), 26-274 (STR), 7-1–7-6 (TSS/TR), 10-1–10-3
(reset)**, and the [Intel 80386 PRM LTR entry](https://pdos.csail.mit.edu/6.828/2018/readings/i386/LTR.htm),
[§7.2](https://pdos.csail.mit.edu/6.828/2018/readings/i386/s07_02.htm),
[§7.3](https://pdos.csail.mit.edu/6.828/2018/readings/i386/s07_03.htm),
[§9.8.10](https://pdos.csail.mit.edu/6.828/2018/readings/i386/s09_08.htm).
The 486 §7.3 sentence calling STR privileged conflicts with its §4.2 table and
instruction exception table; STR has no CPL check. LTR requires CPL 0 before
reading its source. The interpreter retains its general **ring-0-only execution**
boundary; this change does not enable user-mode execution. A handler-level test
checks the architectural CPL distinction independently of that boundary.

LTR requires a non-null **GDT** selector and an available system TSS descriptor:
type 1 for a 16-bit TSS, type 9 for a 32-bit TSS. Null raises #GP(0); TI=1,
out-of-table, busy (3/B), code/data or other system type raises #GP(selector).
A valid type with P=0 raises #NP(selector). Selector error codes clear RPL and
retain TI; source RPL and descriptor DPL otherwise do not restrict LTR. The
whole descriptor must fit the inclusive GDTR limit. All checks finish before
setting bit 1 of the access byte through the PC-98 bus and committing TR and its
hidden base, inclusive limit, access/type/presence and attributes. Cached type
3/B distinguishes 16/32-bit state. No IRQ or device scheduler runs inside this
instruction's busy update/commit; this is a single-CPU emulator.

**LTR is not a task switch:** it does not read or write the TSS, load its saved
registers, set CR0.TS, use a backlink/stack/I/O bitmap, or clear the previous
TSS's busy bit. The 44-byte (16-bit, inclusive limit 43) and 104-byte (32-bit,
inclusive limit 103) minimum sizes apply when **switching tasks**, not when
loading TR. LTR caches even a shorter descriptor; imposing #TS/#GP on that load
would invent an exception absent from the target-era LTR contract. Tests cover
both proper minimum-sized descriptors and shorter cached limits, as well as
page granularity. Actual task use remains explicitly unsupported.

STR reads the visible selector without consulting the GDT or TSS. Changing the
GDT, clearing its busy bit, reloading GDTR or leaving/reentering PE does not
change TR/cache. A later successful LTR replaces the cache; reloading a busy
descriptor faults until guest software clears busy. Construction and machine
reset deterministically choose TR=0 and an uninitialized cache (`None`). This is
an emulator reset policy: the target-era Intel reset tables do not guarantee
initialized TR contents. Reset does not fabricate a descriptor or touch guest
TSS storage. The original fixture rebuilds its table on each boot.

The physical bus has no page/write faults: ROM writes are ignored, and graphics
reads may modify EGC latches. LTR therefore preflights its source word for
side-effecting reads, and every GDT descriptor byte as **ordinary writable RAM
after PC-98 A20 decoding**. A source in latched EGC graphics stops as `LTR source
bus access`; ordinary ROM, RAM and other side-effect-free source reads retain
their bus behavior. A descriptor in ROM, unmapped memory or a device window stops
as `LTR descriptor bus access` before any descriptor read or write. These limits
do not synthesize an architectural protection/page fault or claim a busy update
succeeded. STR memory writes retain ordinary bus/device semantics after segment
preflight. Tests verify rejected spans, descriptor-boundary crossings and EGC
source/pattern latches, including A20 aliases. With paging, these checks use translated physical bytes.

The wholly original MIT [guest](../guest/task_register.asm) executes real →
16-bit protected → 32-bit protected → real → protected → real code. It loads both
TSS types, checks busy bits, register and memory STR, 66/67 word semantics, RPL/DPL
behavior, guest busy clearing/reload, flags and TR retention across PE transitions.
It compares a 64-byte result/guard block and prints `TASK REGISTER PASS` only
on success. Native and production-Wasm browser tests run the exact same disk
cold and after reset, verify the result bytes, untouched TSS storage and disk.
The [manifest](../fixtures/task-register-manifest.json) pins source, boot and disk
SHA-256, MIT license, NASM command, results and a two-million-slot budget. Generated
source/license/media copies remain outside `web/`.

```sh
python3 tools/prepare_task_register_fixture.py
cargo test --locked --lib --test protected_mode --test headless_trace
cargo run --locked --bin headless -- fixtures/task_register/task_register.hdm 10000 trace --ap
npm test
```

## Explicit unsupported paths

Only CPL 0 execution and same-ring transfers are accepted. Privilege returns,
call gates, task gates/switches and IRET task/virtual-8086
returns stop with named protected-mode diagnostics and the restart CPU state.
Invalid direct code/stack privilege combinations raise architectural faults.
No selected fixture needs a privilege change or TSS-based stack switch.
Native tests explicitly check these diagnostic paths without mutating guest memory.
Descriptor-query instructions outside LLDT/SLDT, ARPL, debug-register handling,
NMI/debug-breakpoint machinery, virtual-8086 execution, DPMI, DOS extenders,
Windows and Windows drivers are not implemented here. Existing x87 initialization
and metadata stores work through protected segments; CLTS and WAIT MP/TS checks
are implemented. Arithmetic and pending numeric exception completion remain
unsupported. Synthetic real-mode BIOS services are explicitly rejected
while PE=1; protected handlers execute guest code through IDT gates.

These restrictions do not promote the project's protected-mode/CPU386 inventory
to full architectural fidelity. Unsupported execution terminates diagnostically;
there is no application-name or disk-filename shortcut. More advanced features
need independently demonstrated guests and separate tasks.

## Focused compatibility inventory and cohesive future milestones

This inventory describes code boundaries, not predictions about any unexamined
HDD. None of the original initialization fixtures requires the advanced paths.
New task Issues should be based on an independently reproducible requirement,
with CPU metadata and lawful fixtures; this change establishes no additional
real-guest requirement and creates no speculative Issues.

| Cluster / possible milestone | Current code and test evidence | Cohesive independent fixture group / architecture basis |
| --- | --- | --- |
| **TR/TSS initialization (implemented)** | `system_group0`, `load_task_register`; `tests/protected_mode.rs` LTR/STR matrix and original boot; `task-register.spec.js` cold/reset | Available 16/32-bit descriptors, visible/cache retention, busy updates, selector faults, 66/67, A20; Intel PRM ch.7 and LTR/STR entries. |
| **Descriptor queries and privilege prerequisites** | Group-0 VERR/VERW stop as descriptor queries; 0F 02 LAR / 03 LSL remain unimplemented in `extended`; ARPL has a named boundary. LLDT/SLDT already work in ring 0. | One original query guest with readable/writable/conforming/system descriptors, DPL/RPL/ZF, invalid/null selectors, LDT and size overrides; Intel PRM ch.6/17. Useful as a cluster, not successive guessed opcodes. |
| **Privilege gates and TSS stacks** | `code_target` rejects call gates and privilege returns; `protected_interrupt` rejects a privilege/TSS stack switch. General execution outside CPL0 stops. Initialization does not authorize a ring change. | Coordinated CPL3↔0 interrupt/trap/call-gate and RETF/IRET guests, 16/32-bit SS:SP/ESP TSS fields, complete frame preflight, #TS/#SS/#NP/#GP, IOPL/CLI/STI/POPF, segment invalidation; PRM ch.6/8/9. Implement access rules and all return paths together. |
| **Hardware tasks, task gates and nested return** | `code_target` names direct hardware task switch vs task gate; IDT task gates stop in `protected_interrupt`; IRET.NT stops **before reading a normal frame**. Loaded-TR rejection tests assert intact CPU/RAM. | Original 16/32-bit TSS CALL/JMP/IDT gate/IRET.NT chain with busy/backlink/NT/TS semantics, minimum limits, saved/loaded state, exception ordering and atomicity; PRM ch.7, §9.8.10, ch.21 and instruction operations. No fake host-side task switch. |
| **I/O protection and virtual-8086** | Ring-0 I/O uses existing PC-98 ports; no TSS bitmap is consulted. IRET virtual-8086 return has its own boundary. No nonzero-CPL execution is enabled. | After the privilege milestone, an original IOPL/bitmap guest covering scalar and string I/O, bitmap limits/terminator and denied-access side effects, followed by independent VM86 entry/interrupt/return fixtures; PRM ch.8/15. |
| **CR0.TS / #NM coordination (implemented)** | `extended` CLTS, all ESC EM/TS checks, WAIT MP/TS checks; original `clts.asm`, native CLTS/FPU/trace matrices and `clts.spec.js` cold/reset. | TS-only clear, fault/IRET retry and independent EM rejection; see the [source audit and validation](clts-validation.md). No task switch or numerical FPU is implied. |
| **Debug and remaining cache/system operations** | 0F 21/23 debug-register moves, 0F 24/26 test-register moves, INVD/WBINVD remain unsupported extended forms. INVLPG is implemented with paging. | Coordinated DR/#DB/RF/GD and NMI fixtures only on evidence; data-cache signals remain a separate memory-system boundary. Numerical x87 remains separate. |
| **Paging (implemented bounded subset)** | [Issue #107 contract](#i486-4-kib-paging-issue-107): 4 KiB two-level translation, WP, CR2/#PF, TLB/INVLPG; original native/Wasm guest. | No PSE/PAE/NX, ring-3 execution or cache-cycle fidelity; tables require ordinary writable physical RAM. Task switching and privilege stacks remain separate. |

For a private local retest, use the offline headless command in
[the local HDD diagnostic procedure](dos-compatibility.md#diagnose-a-local-hdd-control-register-failure).
STR/LTR records now include names, opcode/ModR/M and CS:EIP, TR/cache, and, for
register-source LTR, candidate selector/descriptor metadata. Memory sources are
not dereferenced by tracing. Unsupported protected stops include TR/cache and
name task switches, gates, privilege/TSS stacks, IRET.NT and paging separately.
The 100-instruction / 16-control-event bounds still apply; older events can be
evicted. Share only the first blocker and a brief CPU metadata excerpt, never
the image, a memory dump or surrounding disassembly. The private `aliceY3.hdi`
was unavailable and **not tested**. Initialization success promises no game or
DOS-extender compatibility.

## Adjacent system-instruction source audit (Issue #105)

The table separates architectural validity from implemented coverage. Architecture
sources are Intel's [80386 PRM system instruction inventory](https://www.scs.stanford.edu/05au-cs240c/lab/i386/s04_02.htm)
and [instruction reference](https://www.scs.stanford.edu/05au-cs240c/lab/i386/c17.htm),
and the [Intel486 PRM](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143)
chapters 4, 9, 19, 21 and 26. This is a source audit with linked regression
coverage, not a claim that every edge was independently executed on hardware.

| Instructions / architectural validity | Current handler and coverage | Remaining limits / untested edges |
| --- | --- | --- |
| CLTS `0F 06`: real mode; protected CPL0; LOCK #UD | `extended`; [CLTS matrix](../tests/clts.rs), handler privilege matrix in `cpu/protected.rs`, original boot and [browser](../tests/browser/clts.spec.js) | Nonzero-CPL decoding is allowed only to deliver its privilege/encoding fault. General user execution, VM86 and privilege-stack delivery remain unavailable. Reserved REP is ignored by decoder policy. |
| SMSW `0F 01 /4`: real/protected, unprivileged; LMSW `/6`: real/protected CPL0 | `extended`; [CPU386 SMSW tests](../tests/cpu386.rs), CLTS readback/lifecycle guest | LMSW changes only low four bits and cannot clear PE. SMSW stores a word; undefined register upper bits preserved by policy. General nonzero-CPL execution stops before SMSW/LMSW; mixed fault priorities are not exhaustively covered. |
| MOV CR0/2/3 `0F 20/22`: real/protected CPL0, register operand, always 32 bits | `extended`, `commit_cr0`; [reads](../tests/mov_cr.rs), [writes](../tests/mov_cr_write.rs), CPU386 and CLTS matrices | Unsupported CR selectors/memory encodings #UD. Reserved writes stop atomically; PG enables translation. Invalid PG/PE and NW/CD combinations #GP. Other privilege checks rely on the ring-0 execution boundary; CR2/#PF and physical CR3 tables follow the [paging contract](#i486-4-kib-paging-issue-107). |
| SGDT/SIDT `0F 01 /0,/1`: real/protected, unprivileged; LGDT/LIDT `/2,/3`: real/protected CPL0; memory only | `extended`; CPU386 GDTR/IDTR matrices, protected boot/IRQ tests | Six-byte preflight, target-era 24/32-bit base and size/segment cases covered. Nonzero-CPL user execution remains unavailable. Unaligned CPL3 #AC and exhaustive competing exceptions are untested. |
| SLDT/STR `0F 00 /0,/1`: protected only, unprivileged; LLDT/LTR `/2,/3`: protected CPL0 | `system_group0`, `load_task_register`; [protected tests](../tests/protected_mode.rs), task-register boot/browser | LTR marks available TSS busy and caches metadata; it does not switch tasks or set TS. STR always writes a word. LLDT/LDT are bounded and SLDT uses the generic register writer: its operand32 upper-word policy lacks a dedicated target-era regression. General CPL>0 execution is unavailable. |
| VERR/VERW `/4,/5`, LAR/LSL `0F 02/03`, ARPL `63`: protected-only queries/selector adjustment, not CPL0-only | Group-0 queries stop by name; LAR/LSL stop as unimplemented extended opcodes; ARPL names its boundary and #UD in real mode | No ZF/access-rights/limit semantics. Group-0 real-mode #UD is covered; LAR/LSL real-mode #UD is still an unsupported diagnostic, not an implemented instruction contract. No dedicated successful query fixture. |
| MOV DR `0F 21/23`, MOV test registers `0F 24/26`: privileged; INVD/WBINVD `0F 08/09` are privileged i486 memory-system operations | No execution handler; named unimplemented extended/group stops | DR/GD/#DB, test registers, and external cache signals absent. INVLPG has a separate paging implementation. No success coverage. Do not emulate these by returning arbitrary registers or silently accepting flushes. |
| CLI/STI, HLT, PUSHF/POPF, IN/OUT and strings: real/protected with IOPL/privilege rules | `execute`; existing CPU, PIC/PIT, DOS and device suites at real mode/CPL0 | IOPL checks and TSS I/O bitmap are not implemented. Nonzero-CPL execution stops; this cannot be represented as full protected I/O protection. |
| Far CALL/JMP/RET, INT/IRET, task gates and IRET.NT | `far_transfer`, `code_target`, `protected_event`; protected/TSS/IRQ matrices | Same-ring code and IDT gates implemented. Hardware tasks, privilege gates/stacks, nested task return and VM86 stop explicitly. No automatic task-switch TS setting exists because no hardware task switch executes. |
| FNINIT, FNSTCW, FNSTSW memory/AX; WAIT | `execute`; [FPU](../tests/fpu.rs), CLTS and original real/protected boot | EM/TS #NM for all ESC; MP+TS #NM for WAIT. No pending numerical completion, #MF/NE/IRQ routing, numerical stack data or arithmetic. Unsupported numerical operations still stop after TS/EM are removed. Reset clears TS/EM and retains existing environment metadata by the documented reset policy. |

All rows inherit common fetch/prefix limits. Invalid encodings do not authorize
valid but absent subsystems. The CLTS and #NM changes do not extend the privilege,
paging or numerical boundaries. The existing grouped roadmap above remains the
planning boundary, with these priorities when an original guest establishes need:

1. Descriptor query/selector validation as one cluster (LAR/LSL/VERR/VERW/ARPL,
   LDT and store-width edge coverage), followed by coordinated privilege/gate/
   return and I/O protection fixtures if a guest needs them.
2. Debug state/#DB/NMI, or numeric FPU state/exceptions, as separate families
   selected by the next reproducible blocker. WAIT numeric completion belongs
   with FPU error routing, not a fabricated successful WAIT.
3. Hardware tasks/backlinks/busy/NT/TS and privilege stacks as complete transition
   chains; data-cache signals/timing remain separate from the implemented paging/TLB subsystem.

These priorities describe dependencies, not a prediction of the private HDD's
next instruction. The new original guest requires only CLTS/TS/#NM. No larger
independent feature family was demonstrated as a compatibility requirement, so
no speculative follow-up Issues were created. The [CLTS evidence](clts-validation.md)
records the two directly reproduced lifecycle defects fixed within this scope.

## Reproduce the original guest

The wholly original MIT [assembly](../guest/protected_mode.asm) is a single
1024-byte PC-98 IPL sector. It needs no DOS or private media. The
[checked manifest](../fixtures/protected-mode-manifest.json) pins source, boot
and disk SHA-256, assembler command, license, result bytes and budget. Preparation
also copies source/LICENSE beside generated media, outside `web/`.
The guest spells `SMSW EAX` explicitly in 32-bit code: NASM 2.16.01 emits an
extra `66` for `SMSW AX`, while 3.02 omits it. The explicit form produces
identical boot/image bytes with both assemblers and retains manifest verification.

```sh
python3 tools/prepare_protected_mode_fixture.py
cargo test --locked --test protected_mode --test headless_trace --test mov_cr_write
cargo run --locked --bin headless -- fixtures/protected_mode/protected_mode.hdm 10000 trace --ap
npm test
```

The host only mounts, steps, observes and resets. The guest owns the GDT/LDT,
IDT, PIC/PIT programming and all handler/control flow. It executes 16-bit
protected code, flat 32-bit code with EIP and ESP above 64 KiB, both size
prefixes, near/far CALL/RET, LLDT/GS, extended RAM, PC-98 A20 I/O, INT 30h and
a recoverable #GP(40h). The fault handler checks the error/frame and repairs the
saved EIP, then IRETD resumes. A real one-shot PIT IRQ0 remains pending while
IF=0; after STI it runs the guest IDT handler, checks PIC ISR=1, sends EOI,
checks ISR=0 and IRETD returns. Code then restores 16-bit segments, clears PE
and far-jumps back to real mode. It restores real IDTR, checks CR0, compares the
entire 64-byte result/guard block and prints `PROTECTED MODE PASS` through PC-98
text VRAM. Failure is visible and never reports PASS.

The deterministic result at 8000h is 64 bytes initially A5h, with these changes:

| Offset | Bytes | Evidence |
| --- | --- | --- |
| 01–04 | 16 01 01 01 | 16-bit entry, software handler, #GP handler, hardware handler |
| 09–0C | 32 32 32 32 | 32-bit operand/address execution |
| 11 | 67 | 16-bit address override in 32-bit code |
| 19–1B | 55 4E 46 | 16-bit operand override, near and far subroutines |
| 21 | 52 | Return to real mode after CR0 check |

Native and browser tests additionally require `A5 CD AB 34 12 A5` at
210000h, stack guards A5h at 18FE0h/19000h, no host error, unchanged disk bytes
and zero disk writes. Both execute cold and after reset; browser tests use the
actual production release Rust/Wasm at `/` and `/pc98-rust/` with no substitute
JavaScript CPU. Native tests also assert protected CS.D states, EIP > FFFFh,
PIC ISR, saved fault metadata, final cache/base/default size, CR0, GDTR and IDTR.

## Historical Issue #101 validation record

The baseline is checkpoint `7b227425f5bfd90c01e985157a7ec0f35e016fa9`.
An isolated `git archive HEAD` copy of that unchanged source was compiled and
ran the new original disk. It exited 1 before CR0 mutation:

```text
unsupported protected execution/paging transition: CS:IP=1fc0:0000009e
opcode=0f 22 modrm=c0 CR0 GPR=eax old=60000010 written=60000011
PE=1 PG=0 next-ip=000000a1; write not committed
```

The implementation runs that path with PE actually set and completes the guest.
The final disk SHA-256 is
`7d02a3ed1de4fc556b82548477dfa500d46bbaa0add196634eb73eca58f55873`;
source/boot hashes and all reproduction data are in the checked manifest.
The old control-write default guest/image remains unchanged. Its PE-only
variant now commits and halts in the preserved-cache transition window; PE+PG
now commits and faults on its missing page tables, ending in bounded shutdown. Issue #107 supersedes the earlier PG rejection. Tests and manifest distinguish these outcomes.
Historical [Issue #99 evidence](control-register-writes-validation.md) records
the earlier rejection rather than the current protected capability.

Validation used all README fixture preparation commands: independent NASM probes,
checksum-pinned FreeDOS and matching source archives, DOS applications, CPU,
HDD, music, device and font fixtures, plus the new protected fixture. Existing
fixture manifests and published guest assets remained unchanged apart from the
explicit PE/PG outcome metadata.

- `python3 -m unittest discover -s tools -p test_fixture_fetch.py`: 7 passed.
- `python3 tools/build_font.py --check`,
  `python3 tools/prepare_protected_mode_fixture.py` and
  `python3 tools/prepare_mov_cr_write_fixture.py`: passed.
- `cargo test --locked`: **425 passed, 0 failed, 0 ignored** across 28 native/test/doc suites.
- `cargo test --locked --test protected_mode --test cpu386 --test headless_trace
  --test mov_cr_write`: 90 passed, including 18 protected-mode tests. Coverage
  includes protected AAM #DE frames, full-width fetch wrap, indirect far-pointer
  overflow, retained caches after PE clear, and BIOS timer callbacks after return.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `git diff --check` and modified documentation relative-link checks: passed.
- `npm ci` and `npx playwright install chromium`: passed.
- `npm test`: release build and publication audit passed; the default test server
  could not bind occupied port 8000. The unrelated server was left running.
- `npx playwright test --config .issue101-playwright.config.js`: 144 passed in
  3.3 minutes, including VM/V30, Ap/DOS/CPULEVEL, PIC/PIT/FDC/IDE, graphics/sound,
  ADV98/98VIDEOP, Star Trail, HelloWorld and asset checks. This temporary config
  imported the repository config, changed only serving ports to 18190/18191 and
  selected four workers. The project server retained the isolated `web/` symlink
  at `/pc98-rust/`; both roots used this worktree's production release Wasm.
- After final review added protected #DE and pointer-overflow refinements,
  `npm run build` rebuilt release Wasm and passed the publication audit:
  **10 approved assets**, valid 495399-byte `core.wasm`.
- `npx playwright test --config .issue101-playwright.config.js
  tests/browser/protected-mode.spec.js tests/browser/mov-cr-write.spec.js
  tests/browser/mov-cr.spec.js`: **8 passed** against that final artifact.
  Both serving roots and cold/reset runs check exact results/guards and disk
  preservation. The temporary config was removed after the tests.

### Review-fix validation (Issue #101)

The fixture was rebuilt and manifest-verified with both NASM 2.16.01 and 3.02.
Their boot binaries compare byte for byte, with the same boot/image hashes as
before the fix; only the assembly source hash changed. The ENTER regression
failed before the fix for operand16/SS.B=1 and now checks both mismatched sizes,
BP wraparound, `67` independence, complete frame bytes and memory guards.
The real-mode regression also requires #SS for an EBP chain beyond the SS limit.

The fixture, native and build acceptance commands in
[emulator.yml](../.github/workflows/emulator.yml) passed in an isolated
Ubuntu 24.04 arm64 container with the distribution's NASM 2.16.01, Node 22.21.1
and stable Rust 1.99.1, using this worktree's source files:

- All fixture preparation and manifest checks passed, including the protected guest.
- `cargo test --locked`: **426 passed, 0 failed, 0 ignored** across 28 suites.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`: passed.
- Release Wasm build and publication audit: passed, 10 approved assets.

The container's Chromium run stalled and Docker became unresponsive; it did not
complete the browser suite. Browser verification was moved to the native host
with two workers and unused serving ports, preserving both paths and test code.
All fixture preparations and `cargo test --locked` also passed on the host
(426 tests). `npm run build` passed the release build and publication audit
(10 assets, valid 495293-byte Wasm). The temporary Playwright config imported the
checked config, selected two workers and changed only serving ports to
18190/18191; its project server retained the `web/` symlink at `/pc98-rust/`.
The first host browser run had 143 passes and a VM ADD4S boot failure/timeout;
`npx playwright test --config .issue101-playwright.config.js --last-failed`
then passed that case cold and after reset.
A subsequent full `npx playwright test --config .issue101-playwright.config.js`
passed **144 tests in 7.1 minutes**, including the protected guest cold/reset
at both paths, against the same release artifact. The temporary config/server
were removed and `git diff --check` passed. No GitHub-hosted workflow was
triggered from this worktree-only revision; no push or merge was performed.

Review covered the source Issue's bounded subset, descriptor/stack preflight and
fault restart, diagnostic restrictions, preserved V30 decoding, fixture provenance,
CI preparation and publication exclusions. No dependency/core substitution,
application-specific execution shortcut or restricted asset was introduced.

No `aliceY3.hdi` was available or tested. No private disk, source, code bytes,
hashes or traces were acquired, committed, uploaded or published. The original
probe proves this CPU subset, not playability or the next blocker of that game.
No post-merge verification, deployment, physical machine or manual approval is
required for these tests.

### Task-register implementation validation (Issue #103)

The original `task_register.hdm` guest reproduced the prior boundary against the
selected pre-implementation core (`ff0ed0e`): `unsupported protected TSS/task
register; PE=1 PG=0 CS:EIP=0008:00000036`, GDTR `0001fda8/0037`. The original
assembly at that location is `LTR AX`, `0F 00 D8`, selector 0028h. This locates the
new legal fixture's blocker; it does **not** identify the private HDD's opcode.
The implemented core executes the same boot/image bytes through to
`TASK REGISTER PASS` (655 retired instructions in the native headless run).

The new native matrix covers both TSS types, RPL/DPL, minimum-sized and shorter
limits, page granularity, null/TI/busy/type/presence/GDT bounds and selector error
codes; every GPR and both default sizes; repeated 66/67; memory/segment overrides;
upper register preservation; flags; cached metadata after descriptor edits;
ROM source reads; PC-98 A20 aliases; reset; and a guest #NP handler that repairs
the descriptor and IRETD retries the first prefix. Rejected descriptor/source
spans and EGC accesses preserve memory and source/pattern latches. Truncated
instruction decoding cannot reach a device operand. Loaded TR does not enable
hardware tasks, task/call gates or IRET.NT; NT stops before a normal frame read.
A separate privilege/TSS interrupt-stack boundary test preserves CPU/RAM, and the
existing paging, privilege-return, V30, real-mode and protected IRQ tests remain.

Final validation in the assigned worktree on 2026-10-29:

- All fixture preparations and asset checks from `emulator.yml` passed (43
  preparation/asset commands), including the legal FreeDOS/source preparations.
  `python3 -m unittest discover -s tools -p test_fixture_fetch.py`: 7 passed.
  The new fixture was regenerated and then verified without `--update-manifest`;
  image SHA-256 is `31befb0a3250873829b77d9e2f6199888b830824a079eca372d4089569bf1946`.
  Explicit STR/LTR bytes avoid assembler operand-prefix choices; NASM 3.02 was
  used for this run.
- `cargo test --locked`: **452 passed, 0 failed, 0 ignored**, including the
  instruction-handler privilege test, 43 protected-mode tests and six trace
  tests. `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check` passed.
- `npm ci` and `npx playwright install chromium` passed. `npm test -- --config=.issue103-playwright.config.js` passed the production release Wasm
  build, publication audit (**10 approved assets, valid 500913-byte Wasm**) and
  **all 146 Chromium tests in 4.6 minutes**. The new task-register guest passed
  cold/reset at both `/` and `/pc98-rust/`, with exact manifest result/guard bytes,
  untouched TSS storage, unchanged exported disk and no browser/emulator error.
- Port 8000 was already occupied. The temporary config imported the checked
  Playwright config, used two workers, and changed only serving ports to
  18290/18291. Its project server used the checked `serve_project.py` with the
  alternate port and retained the `web/` symlink under `/pc98-rust/`. Test code
  and both serving paths were preserved; temporary config/server files were
  removed afterward. No existing server was stopped or modified.
- Updated relative documentation links were checked. No Cargo/npm dependency,
  emulator library, binary core, host-side guest result substitution, private
  image or proprietary ROM/font was introduced. The generated probe stays
  outside the publication tree. No push, PR, merge, comment or deployment was
  performed; none is required to validate this node.

The private `aliceY3.hdi` was unavailable and was not tested. No claim is made
about its next blocker or playability. No post-merge verification is required.
