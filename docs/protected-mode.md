# Ap paging-disabled protected mode (Issue #101)

PC-9821Ap DOS now executes an original-Rust ring-0 protected subset with
CR0.PE=1 and CR0.PG=0. The default VM/V30 decoder and devices retain their
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
  SMSW and existing x87 EM/TS checks share that state. CR2/CR3 retain their
  existing metadata semantics. LMSW changes low control bits but cannot clear
  an already set PE. PG without PE and NW without CD raise #GP before mutation.
  Any otherwise valid PG=1 request terminates with `unsupported paging transition`;
  reserved writes retain their explicit undefined-bit diagnostic. Both terminal
  write rejections restore the complete CPU snapshot and leave the bus unchanged.
- **Transition window:** PE changes protection checks without replacing hidden
  CS/SS/data attributes. A far transfer reloads CS from the descriptor table.
  Clearing PE retains caches until segment reloads; a real data load changes
  the base to selector × 16, retaining the cached limit/B. A real far code
  transfer restores conventional code access/default size and replaces the base,
  retaining the limit. The accepted return first loads 16-bit, byte-granular,
  writable 64 KiB stack/data and 16-bit 64 KiB code, disables interrupts, clears
  PE, far-jumps to real code, reloads IDTR and then reloads real data segments.
  No guest PE bit is silently cleared by the emulator. Reset clears caches,
  LDTR and fault metadata and restores real-mode control/table reset state.
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
  Linear base+offset wraps at 32 bits and goes directly to the existing PC-98
  physical bus, including 4 MiB RAM, device windows, ROM write protection and
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
  recent fault vector/error code. Only opcode/ModR/M and CPU metadata are read;
  no surrounding guest code or operand memory is dumped.

## Explicit unsupported paths

Only CPL 0 execution and same-ring transfers are accepted. Privilege returns,
call gates, TSS/task registers, task gates/switches and IRET task/virtual-8086
returns stop with named protected-mode diagnostics and the restart CPU state.
Invalid direct code/stack privilege combinations raise architectural faults.
No selected fixture needs a privilege change or TSS-based stack switch.
Native tests explicitly check these diagnostic paths without mutating guest memory.
Descriptor-query instructions outside LLDT/SLDT, ARPL, CLTS, debug-register handling,
NMI/debug-breakpoint machinery, paging, virtual-8086 execution, DPMI, DOS extenders,
Windows and Windows drivers are not implemented here. Existing x87 initialization
and metadata stores work through protected segments; arithmetic and WAIT completion
remain unsupported. Synthetic real-mode BIOS services are explicitly rejected
while PE=1; protected handlers execute guest code through IDT gates.

These restrictions do not promote the project's protected-mode/CPU386 inventory
to full architectural fidelity. Unsupported execution terminates diagnostically;
there is no application-name or disk-filename shortcut. More advanced features
need independently demonstrated guests and separate tasks.

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

## Validation record

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
continues to stop atomically. Tests and manifest distinguish these outcomes.
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
