# CLTS and CR0.TS / #NM validation (Issue #105)

The Ap interpreter implements `0F 06` CLTS and repairs two adjacent #NM paths.
This proves an original CPU guest, not boot or playability of a private program.
The optional `aliceY3.hdi` was unavailable and **not tested**. No post-merge
verification, live deployment, physical machine or human playtest is required.

## Architecture and encoding

The primary references are Intel's [1992 Intel486 Programmer's Reference Manual](https://manuals.plus/m/d86207aaebd9b10464f2058efc61559e6141d3b072db100e89bfe17c8187e143)
(CLTS p.26-55, WAIT p.26-280, control registers §4.1.3, exception 7 §9.9.7,
reset §19.2), and the independent [80386 PRM CLTS entry](https://www.scs.stanford.edu/05au-cs240c/lab/i386/CLTS.htm).
Both specify TS-only clearing, real-mode permission and protected CPL0 privilege.
Intel486 also specifies #GP(0) in virtual-8086 mode; that mode remains unsupported.
The older 386 HTML CLTS entry's virtual-8086 exception text conflicts with the
486 entry; it is not used to authorize privileged virtual-8086 execution.

`CLTS` has no ModR/M, immediate, memory operand or flag result. `0F 06 B0 5A`
is CLTS followed by MOV AL,5Ah. `66 67 64 0F 06` is a five-byte CLTS:
size overrides and segment selection have no operand to affect. The common
15-byte instruction limit and CS fetch checks still apply. `F0 0F 06` faults
#UD before changing CR0, following Intel's [386 LOCK rules](https://www.scs.stanford.edu/05au-cs240c/lab/i386/LOCK.htm).
Repeated 66/67 follow the existing decoder's override policy. REP/REPNE on this
non-string instruction are reserved, rather than a promised legal encoding;
Ap retains its existing ignore policy and no extra REP legality is claimed.

`extended` checks CPL before clearing bit 3 directly. It does not call the CR0
mode-transition helper, force ET, clear EM or touch hidden caches. The outer
nonzero-CPL boundary permits decoding CLTS solely to deliver #GP(0), or #UD for
LOCK; other instructions still stop with the existing ring-0-only diagnostic.
Native probes deliver these faults through same-ring gates at CPL1/2/3 without
inventing privilege-stack switching. A gate requiring a ring transition still
hits the named TSS-stack limitation. Fault restart uses the existing saved CPU
and IDT/IVT machinery, including the first prefix and protected RF/error frame.

## Coupled defects demonstrated and fixed

The original regressions in [tests/clts.rs](../tests/clts.rs) were also compiled
against an isolated archive of the unchanged selected checkpoint
`3d898e613c44c942bb55255357d7b5ab479368c1`:

- `wait_mp_ts_matrix_and_pending_exception_boundary`: with MP=TS=1, the old
  WAIT advanced IP to 1 instead of entering exception 7 at 2000h. WAIT now
  checks MP AND TS; EM alone does not fault. Pending numeric completion remains
  unsupported and stops explicitly instead of silently completing.
- `esc_nm_precedes_unsupported_numerics_without_fabricating_results`: the old
  `D8 C0` stopped as `unsupported opcode d8` even with EM/TS set. All D8–DF ESC
  opcodes now check EM/TS before the implementation whitelist. After removing
  those bits, unsupported numerical instructions still stop with an honest
  `unimplemented x87 opcode` diagnosis; no mathematical result is fabricated.

Existing FNINIT, FNSTCW and both FNSTSW forms retain their non-waiting metadata
semantics. Exception 7 precedes their state/store changes. Numerical exception
completion, #MF/NE/IRQ routing and asynchronous FPU timing are not implemented.

## Original fixture and pre-fix evidence

The [MIT assembly](../guest/clts.asm), [offline builder](../tools/prepare_clts_fixture.py)
and [checked manifest](../fixtures/clts-manifest.json) contain no third-party CPU,
ROM, font, OS or game data. The single 1024-byte IPL sector is padded to a standard
PC-98 floppy. Source, boot and full-media SHA-256 are pinned; generated media,
source copy and LICENSE stay in ignored `fixtures/clts/`, outside publication.

```sh
python3 tools/prepare_clts_fixture.py
cargo test --locked --test clts --test fpu --test headless_trace
cargo run --locked --bin headless -- fixtures/clts/clts.hdm 10000 trace --ap
npm test
```

The identical original disk on the unchanged selected checkpoint stopped with:

```text
mode=real CS:EIP=1fc0:00000023 opcode=0f 06 CR0=60000010 TS=0
instructions=137 fdc_reads=1 fdc_writes=0 next-CS:EIP=1fc0:00000025
unimplemented i486 opcode 0F 06
```

This TS=0 is the original probe's state, not a deduction about the private HDD.
The implemented core completes `CLTS PASS`. Each real/protected path sets TS
using LMSW and MOV CR0, takes #NM on FNINIT, FNSTCW, memory FNSTSW and FNSTSW AX,
and takes WAIT #NM with MP=TS=1. Guest handlers check the saved fault address and
CS (also RF in protected mode), execute CLTS, and IRET/IRETD retries the original
instruction. Finally the guest sets EM+TS, executes CLTS, verifies EM survived,
and takes a further FNINIT #NM. That handler explicitly clears EM before retry.
Both paths compare CR0 and SMSW readback. No host substitutes any instruction,
handler, FPU result or guest branch.

The result/guard block at 8000h is 64 bytes initially A5h. Each 16-byte real
(offset 0) and protected (offset 16) result region changes only:

| Relative offsets | Bytes | Meaning |
| --- | --- | --- |
| 1–2 | 06 01 | Six #NM frames, one caused by EM after CLTS |
| 4–7 | 7F 03 00 00 | Initialized FPU control/status words |

The other 52 bytes remain A5h. Stack guards at 7B00h and 7C00h remain A5 A5.
The guest compares all 64 result bytes before printing PASS. Native tests also
check final CR0=60000011h, full initialized FPU metadata, ESP=7C00h and #NM
metadata. [Production-Wasm browser tests](../tests/browser/clts.spec.js) verify
identical markers, full block, guards and unchanged exported media cold/reset
at both serving roots. Native media checks additionally require zero disk writes.

The native CLTS matrix covers TS/EM combinations, MP/NE/WP/AM/CD/NW/ET/PE
preservation, both protected default sizes, real mode, GPRs/flags, CR2/CR3,
segment/GDT/IDT/LDTR/TR caches, RAM/graphics, reset, MOV/SMSW/LMSW agreement,
15/16-byte encodings, CS-limit fetch, LOCK and #GP restart. Handler-level tests
also compare the complete CPU state at all CPLs, excluding only fetched opcode
position/count and the successful TS change. The separate public-step privilege
matrix checks actual #GP/#UD frames and preserves the general user-execution stop.
Existing FPU tests seed every environment field and cover all supported stores'
exact widths, relocated vectors, #NM/#UD/#GP and IRET retry.

## Bounded diagnostics

Headless tracing now retains CLTS in the existing 16-control-event history,
separate from its 100-instruction history. A CLTS record includes mode, CS:EIP,
opcode/prefix metadata, old CR0/TS and the **actual** post-step CR0/TS, execution
result, next CS:EIP and last fault metadata. Result distinguishes execution,
delivered fault and host stop using instruction retirement, not a predicted
CR0 change. The last-fault field is historical and may describe an earlier
exception; it is not proof that a successfully retired CLTS faulted. Real IVT
fault delivery now records its vector with error code zero in the same field. No ModR/M is read for
CLTS and no memory operand or surrounding guest code is dumped. The original
trace regression checks successful clearing and LOCK-fault nonmutation.

## Validation record

Validation ran locally in the assigned worktree on 2026-10-29 (NASM 3.02).

- All 44 existing Python preparation/asset commands from `emulator.yml` passed,
  including seven fixture-fetch unit tests, checksum-pinned FreeDOS binaries and
  corresponding sources, DOS applications, CPU/HDD, music/device probes, media
  and font checks. The additional `python3 tools/prepare_clts_fixture.py` passed
  manifest verification after generation. Existing fixture assets/manifests did
  not change. Explicit SMSW/LMSW bytes avoid assembler-dependent size prefixes.
- The unchanged selected-checkpoint archive reproduced the original CLTS stop
  and both coupled regression failures. The final original image SHA-256 is
  `600820dd862f05f2f2919a7873b6b0857b9c5a4b631629ce12be1142dedf5562`.
  `cargo run --locked --bin headless -- fixtures/clts/clts.hdm 10000 trace --ap`
  prints `CLTS PASS`, 588 retired instructions, one FDC read and zero writes.
- `npm ci` and `npx playwright install chromium` passed. The initial full browser
  run passed 148 tests before the final privilege and diagnostic refinements.
- An intermediate full native run found two MOV-CR-write snapshot assertions
  expecting empty real-mode diagnostic metadata. Their expected state now
  explicitly records #UD/#GP and still compares every CPU field and RAM byte;
  `cargo test --locked --test mov_cr_write --test clts --test headless_trace`
  passed all 25 tests after that correction.
- Final `npm test -- --config=.issue105-playwright.config.js` passed the release
  Rust/Wasm build, publication audit (**10 approved assets, valid 501202-byte
  Wasm**) and **all 148 Chromium tests in 4.6 minutes**. CLTS passed cold/reset
  at `/` and `/pc98-rust/` with exact result/guard bytes, no browser/emulator
  error and byte-identical disk export. Existing VM/Ap, protected mode, TR/TSS,
  PIC/PIT/FDC/IDE, DOS/ADV98/98VIDEOP, graphics/sound and publication tests passed.
- Final `cargo test --locked`: **463 passed, 0 failed, 0 ignored**, including
  nine CLTS integration tests, the full-state handler privilege matrix and seven
  bounded trace tests. `cargo clippy --all-targets -- -D warnings` and
  `cargo fmt --check` passed against the final source.

Port 8000 was already occupied. The temporary Playwright config imported the
checked config, selected two workers and changed only ports to 18390/18391.
Its second server retained the checked `web/` symlink under `/pc98-rust/`.
The tests and both serving paths were preserved; no existing server was stopped
or modified. Temporary config/server files were removed after the final run.

Relative documentation file links and heading anchors, source/boot/media hashes,
and `git diff --check` passed. No dependency, emulator/chip library, host CPU
substitution, proprietary ROM/font, private image or copyrighted game artifact
was introduced. The strict publication audit excludes the test media and source.
No GitHub-hosted CI was triggered, and no push, PR, merge, comment, Issue creation
or deployment was performed. The private HDD remains unavailable and untested.
