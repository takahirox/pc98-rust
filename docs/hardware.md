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

An instruction or one REP element consumes one 8-clock scheduling slot. This is a
deterministic work/cycle model, **not instruction-cycle accuracy**. Firmware FDC
transactions additionally advance the same device scheduler. HLT stops instruction
retirement while clocks and IRQs continue. STI and MOV/POP SS inhibit delivery for
the next instruction. A frame runs at most two slices of 16,667 slots; the core
caps each call at 100,000 slots. There is no unbounded catch-up after a suspended
tab. This bounds UI work even for infinite loops and maximum-length REP strings.

## Device ports and interrupts

Port facts are traced to the individual chapters of
[UNDOCUMENTED 9801/9821 Vol.2](https://www.webtech.co.jp/company/doc/undocumented_mem/index.html).
Registers below implement the behavior needed by the three accepted workflows;
this table is not a claim of complete chip emulation.

| Ports (hex) | Device and supported behavior | IRQ / notes |
| --- | --- | --- |
| `00,02`; `08,0A` | Master/slave 8259: ICW vector/cascade initialization, masks, IRR/ISR, priority, specific/nonspecific EOI, auto EOI | Master IR7 cascades slave; masked/in-service IRQs cannot bypass priority |
| `01–0F` odd, `11–1F` odd, `21,23,25,27` | 8237 address/count flip-flop, channel masks, direction/decrement, terminal count, bank registers | FDC uses channel 2; count/address change with actual byte transfers |
| `31,33,35,37` | PPI startup inputs, output C, bit set/reset, active-low beep gate | Input straps are fixed VM-profile constants |
| `41,43` | Keyboard byte FIFO, ready status, command acknowledgements, down/up scan bytes | IR1 / INT 09; firmware maintains BIOS ring, held-key bitmap and modifier state |
| `60,62` | Master text GDC parameter/status and command/data readback | RESET, display enable, PITCH, CURS, CCHAR, PRAM start, CSRR |
| `A0,A2` | Slave bitmap GDC command/parameter/status/readback | Same register subset; 640×400 direct VRAM path |
| `68` | Text display enable | Fixed 80-column 25-row renderer |
| `6A,A8,AA,AC,AE` | Digital 8-color pairs; analog index and 4-bit RGB levels when enabled | Eight palette entries only; no fourth graphics plane / 16-color board |
| `A4,A6` | Display and CPU graphics page select | Two independent three-plane pages |
| `7C,7E` | GRCG mode/tile writes | TDW and RMW, plane masks; no EGC |
| `71,73,75,77`; `3FD9–3FDF` odd | 8253 channels, control/access modes, low/high writes, counter latches | Channel 0 IR0 / INT 08; channel 1 beep; VM input clock 1,996,800 Hz |
| `90,92,94` | 2HD FDC status/FIFO/control, SPECIFY, sense status/interrupt, recalibrate, seek, read ID, READ/WRITE DATA through DMA, motor/DMA-enable gating | IR11 / INT 13; delayed command completion and seven-byte transfer results |
| `BE` | Fixed 1 MB floppy interface selection | No alternate 640 KB controller at C8/CA |
| `40,5C,5E,64,6E` | Fixed system/display inputs and deterministic clock status | See `Bus::input`; no RTC serial protocol |
| `F0` | Private synthetic-firmware thunk ABI, accepted only from its ROM stubs | Not a guest DOS API or claim of a real VM hardware port |

The hello source uses `OUT 3FDFh,AL`, an immediate-port encoding that truncates to
`DFh`. That timer mirror is decoded too. Full PIT gate/readback/mode waveforms,
DMA auto-init reload/arbitration, precise FDC motor/rotation timing, FDC programmed
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
| INT 1B low AH=0,3,4,7,A | Reset/recalibrate, sense and read ID; unavailable drive reports carry/error |
| INT 1B low AH=1,5,6 | Verify/read, write, read; CHRN and ES:BP/BX byte-count contract; uses controller/DMA |
| INT 1C AH=0,2 | Fixed deterministic BCD RTC query (2026-01-01) and one-shot interval callback (see below) |
| INT 1F | Unsupported extended memory, returns carry set |

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

Guest DOS/TSR vectors still run first: chaining to the original INT 09 handler
updates the table; consuming bytes without chaining leaves BIOS state to the
guest handler. No scheduler or host-input shortcut writes the table. Machine
creation/reset initializes all 16 bytes to zero and discards held/pending keys,
modifiers and ring contents, consistent with the existing fresh-device reset.
This is the emulator's initialization policy; it does not reconstruct keys
physically held across a real hardware reboot.

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
