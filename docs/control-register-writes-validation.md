# Issue #99 control-register write validation

This is historical evidence for Issue #99. Issue #101 supersedes the PE-only
rejection with [paging-disabled protected mode](protected-mode.md); PE+PG still
stops atomically. The default write probe and its image hash remain unchanged.

The candidate starts from `8b70e54c87b740469877cc387385712b9b0576fa`.
The final checkpoint commit containing this document identifies the tested source.
The profile for the new guest is **PC-9821Ap DOS**, using an original MIT floppy
IPL, no DOS, proprietary ROM or third-party emulator core.

Before changing the CPU, the newly assembled default write probe was executed:

```sh
python3 tools/prepare_mov_cr_write_fixture.py --update-manifest
cargo run --locked --bin headless -- fixtures/mov_cr_write/mov_cr_write.hdm 10000 --ap
```

It exited 1 with `unimplemented i486 opcode 0F 22`. The same guest subsequently
prints `MOV CR WRITE PASS`. Its checked manifest pins the source and all three
images; ordinary preparation verifies these hashes instead of rewriting them.
The default HDM SHA-256 is
`7625004436de2a560d7be5998dcfb32b5685e2a8506b2032ff65bfcb0a36e467`.

The expected values have separate guest literal, Python manifest and Rust test
oracles: CR2 results from EAX/ECX/EDX/EBX/EBP/ESI/EDI are 12340000h + register
index; ESP supplies 12345018h to CR2 and CR3; CR0 reads 60050032h, SMSW reads
0032h, FLAGS remains 08D7h, four guest faults repair and retry, and every other
result/guard byte remains A5h. The guest compares the entire 128-byte result
block before printing PASS. Browser code only mounts/steps/observes/resets the
actual release Rust/Wasm artifact; it does not interpret the probe or synthesize
results. Both cold boot and reset must produce the same output and unchanged media.

Native instruction tests additionally exhaust all invalid ModR/M/LOCK forms,
source GPR/CR/prefix combinations, defined CR0 bits and reserved bits, check fixed
ET, full register/segment/table/FPU/flags/RAM preservation, real #UD/#GP frames and
IRET retry, CS/15-byte limits, TF, FPU EM/TS, reset and VM decoding. Rejected mode
requests compare the entire CPU snapshot and RAM across repeated steps.

A bounded trace of the synthetic PE image was captured with:

```sh
git rev-parse HEAD
cargo run --locked --bin headless -- fixtures/mov_cr_write/pe.hdm 10000 trace --ap
```

The relevant original-guest metadata (omitting unrelated register fields) is:

```text
profile=Pc9821ApDos instruction-history=100 control-history=16
control-slot=62 1fc0:00000007 opcode=0f 22 modrm=c0 mod=3 cr=0 gpr=eax
old=60000010 written=60000011 PE=0->1 PG=0->0 prefixes=[66, 67, 65]
operand=32 address=32 next-attempt=<stopped>
unsupported protected execution/paging transition: CS:IP=1fc0:00000007 opcode=0f 22 modrm=c0 CR0 GPR=eax old=60000010 written=60000011 PE=1 PG=0 next-ip=0000000d; write not committed
```

This image intentionally requests PE. CR0 remains 60000010h and execution stops
at the first prefix; sequential IP 000Dh is diagnostic only, never executed.
The PE+PG image independently requests E0000011h with the same atomic boundary.
The trace regression also executes a safe CR2=DEADBEEFh write and observes the
following NOP at 1FC0:0009 before a later synthetic PE request. IRQ-entry trace
coverage and the 100-slot/16-control-event bounds remain tested.

No legitimately accessible copy of `aliceY3.hdi` was supplied in this worktree;
its optional retest was **not performed**. Synthetic transitions establish no
new protected-mode requirement for that game, so no follow-up Issue was created.
No claim of complete HDD boot, game compatibility or playability is made.
Private media, payloads and traces are excluded from tracked and hosted assets.

Validation completed successfully:

- All README fixture preparation commands, including checksum-pinned FreeDOS,
  source archives, DOS applications, CPU/HDD/music/device/font fixtures, and
  `python3 tools/prepare_mov_cr_write_fixture.py`: passed. Fixture-fetch unit
  tests: 7 passed; `python3 tools/build_font.py --check`: passed.
- `cargo test --locked`: **405 passed, 0 failed, 0 ignored** across 27 test/doc
  suites. This includes existing VM/Ap, DOS, BIOS, HDD, sound and graphics tests.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check`: passed.
- `npm ci` and `npx playwright install chromium`: passed.
- `npm test`: production Wasm build and publication check passed; browser startup
  was blocked because an existing unrelated server occupied port 8000. That server
  was left running. The complete browser suite was then run with a temporary
  configuration importing `playwright.config.js`, changing only the two serving
  ports to 18090/18091 and setting four workers. The project-path server retained
  its original isolated `web/` symlink behavior.
- `npx playwright test --config .issue99-playwright.config.js`: **144 passed**
  in 2.7 minutes, exercising actual release Wasm at `/` and `/pc98-rust/`.
  This includes the default write guest and PE/PE+PG variants cold and after reset.
- Final review removed misleading PE/PG labels from rejected **CR3** diagnostics;
  those labels now describe CR0 only. After that diagnostic-only refinement,
  `cargo test --locked --test mov_cr_write --test headless_trace` passed all
  **12 tests**; clippy and formatting passed again. `npm run build` rebuilt the
  release Wasm and passed the publication check (10 approved assets).
- `npx playwright test --config .issue99-playwright.config.js
  tests/browser/mov-cr-write.spec.js tests/browser/mov-cr.spec.js`: **8 passed**
  against that final release artifact, at both roots and cold/after reset.
  The temporary port configuration was removed after testing.

Automated diff review checked the Issue's bounded behavior, fault/restart and
atomic stop paths, original MIT probe/manifest provenance, CI preparation, and
unchanged VM decoding. Cargo/npm dependency files are unchanged; no external CPU,
emulator or chip implementation, restricted media or test-only hosted assets were
added. No deployment, human test, proprietary disk or post-merge check is required.
