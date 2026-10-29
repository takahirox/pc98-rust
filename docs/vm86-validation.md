# Independent VM86 validation (Issue #109)

This record concerns the original Rust i486 CPU and wholly original MIT guest.
It does not identify the private HDD's monitor, validate its frame, or establish
DOS-extender, Windows or game compatibility. Private `aliceY3.hdi`: **not retested**.
No private image, copyrighted code/disassembly, memory dump, ROM/font dump or
foreign emulator implementation was obtained or added. No push, PR, comment,
merge or deployment is needed for this worktree's acceptance.

## Reproduction and provenance

Follow every fixture preparation command in [README](../README.md#reproduce-the-checks),
then:

```sh
python3 tools/prepare_vm86_fixture.py
cargo test --locked --lib --test vm86 --test dma --test headless_trace
cargo run --locked --bin headless -- fixtures/vm86/vm86.hdm 100000 trace --ap
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npm ci
npx playwright install chromium
npm test
```

The [builder](../tools/prepare_vm86_fixture.py) assembles
[guest/vm86.asm](../guest/vm86.asm) offline using NASM, verifies the checked
[manifest](../fixtures/vm86-manifest.json), and copies the assembly, MIT LICENSE
and manifest beside the generated disk. Source, license, 8192-byte boot/payload
and 1,261,568-byte disk SHA-256 are pinned. The disk is test-only and excluded
from `web/` and Git. The license hash is checked during every preparation.
The disk SHA-256 is
`f1acc07f3797920296c24c1709e228ad5ad4375569447e785b70aaa325e26267`.
The preflight loader uses the repository's ordinary real-mode disk BIOS before
PE entry. All protected/VM86 guest and handler code executes CPU instructions;
no host function supplies monitor results, emulates DOS, or changes guest modes.

Independent architectural sources are the
[Intel486 PRM (1992)](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143),
chapters 8, 9, 23 and the instruction entries, and Intel's
[80386 PRM](https://pdos.csail.mit.edu/6.828/2018/readings/i386.pdf).
The i486 text takes precedence over early 386 descriptions, including LOCK,
IOPL-sensitive instructions, two-byte bitmap reads and the VM86 return frame.
See [the implementation contract](protected-mode.md#i486-virtual-8086-mode-issue-109).

## Baseline reproduction

An isolated `git archive` copy of unchanged checkpoint
`e959a7bdcc919573b8321e29389ac035cbb5e892` was compiled and booted with the
same final original disk. It exited 1 at the intentionally valid CPL0 IRETD:

```text
unsupported protected IRET virtual-8086 return
PE=1 PG=0 CS:EIP=0008:00020250 CPL=0 CR0=60000011
GDT=00020638/001f IDT=0000c000/018f TR=0018
TSS base=00009000 limit=00002080 access=8b
```

This reproduces the source Issue's old diagnostic independently of the private
HDD. The complete entry frame contains VM=1, IOPL=0, CS=4000h, EIP=0,
SS=6000h, ESP=ABCD7000h, ES=A000h, DS=5000h, FS=0800h and GS=9000h.
The implemented core executes this same frame with PE still set and CPL3.
Native observation checks VM86 in both paging states, rather than inferring
execution from the VM bit alone.

## Guest and native expectations

Each phase checks its restored segments/ESP, 32-bit operands and addresses,
PUSHF/POPF IOPL preservation, bitmap-denied OUT at IOPL3, permitted peripheral
input, cross-page data, and a prefixed instruction spanning two code pages.
An initial CLI raises #GP; the guest monitor validates the complete frame,
grants IOPL3 and resumes. A second #GP catches denied OUT before its A20 effect.
VM86 writes actual PC-98 text VRAM. A real PIC/PIT IRQ0 enters a CPL0 handler
on guest TSS SS0:ESP0; it checks saved IF and PIC ISR and issues EOI. A software
trap gate separately checks that IF survives. Handlers inspect saved context,
run guest instructions and IRETD back to VM86.

The second phase enables PG and WP, with separate physical code, data and
stack pages. Three user faults check CR2 and error codes 4 (absent read),
5 (supervisor read) and 7 (read-only write). Guest handlers repair PTEs,
INVLPG and IRETD to retry. The first-prefix EIP and full context are preserved.
Page tables are walked physically; native FDC/DMA tests additionally verify
physical transfers while the VM86 CPU reads a different nonidentity page.

The exact 64-byte result at 8000h is pinned in the manifest:

| Offset | Expected value | Meaning |
| --- | --- | --- |
| 0–5 | `02 04 03 02 02 01` | Two phases, four #GP, three #PF, two IRQ, two software handlers, PG phase |
| 8 | E0010011h | Guest monitor CR0: PE/PG/WP retained |
| 12 | 00055000h | Last recoverable user #PF address |
| 16 | 12345678h | Actual VM86 operand/address execution |
| 20 | 00023046h | Saved VM86 EFLAGS, including VM and IOPL3 |
| 24 | ABCD7000h | Full saved VM86 ESP |
| 28–48 | 4000h, 6000h, A000h, 5000h, 0800h, 9000h | Saved CS/SS/ES/DS/FS/GS, each a dword |
| 52, 56 | 2, 0 | Guest EAX and EDX at final monitor entry |
| Other bytes | A5h | Result guards |

Both test layers require the entire result and dword guards at 17F80h,
18000h, 66FF0h, 67000h, 116FF0h and 117000h, unchanged disk bytes and no
emulator error, cold and after reset. Native tests also inspect live CPL,
segments/caches, CR0/CR2, halt/shutdown, zero disk writes and both VM86 phases.
Browser tests load the actual production release Wasm and run the same disk at
`/` and `/pc98-rust/`, without a JavaScript CPU or substitute monitor.

Native matrices independently cover complete/truncated entry frames, every
truncated byte of the 36-byte stack span, EIP limits, 16-bit stacks with full
ESP, raw zero segments, IRET width/CPL restrictions, flag privilege masks,
software/hardware/error frame layouts, guest handler return, privileged and
protected-only opcodes, cross-page fetch/data faults, user graphics protection,
INS preflight, TSS/stack/descriptor error codes and external bits, and bounded
nested/double/triple faults with intact memory/latches. I/O coverage includes
permitted/denied byte/word/dword scalar and string forms, bitmap relocation,
supervisor reads, short/missing/16-bit TSS, byte/port boundaries and sentinel
wrapping. REP OUTS retains completed elements when the next element faults.
Trace tests enforce 100 instruction/16 control-event bounds and inspect only
CPU mode, frame span, TSS stack, gate and fault metadata.

## Validation in the assigned worktree

Final checks in the assigned worktree on 2026-10-29:

- All **47** documented Python validation/preparation commands from
  `emulator.yml` passed, including the seven fixture-fetch tests, original
  MIT probes, pinned legal FreeDOS binaries and matching source archives,
  DOS application/CPU/HDD/music/device/font preparation and media/font audits.
  VM86 preparation also passed again without `--update-manifest`.
- `cargo test --locked`: **511 passed, 0 failed, 0 ignored**, across 31
  native/test/doc suites. This includes 15 CPU unit tests, 18 VM86 integration
  tests, eight headless trace tests and physical DMA checks; all existing
  Ap paging, STR/LTR, CLTS/TS/#NM, protected IRQ, real-mode and V30 suites pass.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`,
  `git diff --check`, and modified documentation relative-link checks passed.
- `npm ci` passed, with zero audit vulnerabilities;
  `npx playwright install chromium` completed successfully.
- `npm test -- --config .vm86-playwright.config.js` passed the production
  release Rust/Wasm build, publication audit (**10 approved assets**, valid
  **520125-byte Wasm**) and **all 152 Chromium tests in 5.1 minutes**.
  This was a complete second browser run after final instruction validation
  changes; the preceding complete run also passed 152 tests.
- Port 8000 was occupied. The temporary config imported the checked config,
  selected two workers and changed serving ports to 18490/18491. A temporary
  copy of the checked project-path server changed only its port and preserved
  the isolated `web/` symlink under `/pc98-rust/`. Both serving paths, the
  production artifact and test code remained intact. Temporary configuration
  and server files were removed after validation; no existing server was
  stopped or modified.

Review checked the Issue's entry, execution, paging, monitor, IOPL/bitmap,
atomicity, diagnostics, fixture provenance and validation requirements against
the final diff. No unresolved test regression remains. No dependency or external
CPU core was added. Generated VM86 media and source/license copies remain outside
the publication tree. No post-merge verification, live Pages publication, private
media or manual human gameplay is required.
