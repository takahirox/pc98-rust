# Synthetic BIOS services in VM86 — Issue #118

The original Rust synthetic BIOS now accepts supported VM86 requests with PE=1,
including PG=1. Ordinary protected-mode requests still report
`synthetic real-mode firmware requested from protected mode`.
PR #119 established this independent service baseline without accessing private
media. Its integration with the Issue #113 correction preserves both original
fixtures and the BIOS-only scatter path. The separate
[HDD investigation](hdd-boot-investigation.md#current-restart--2026-10-30-bootinput-correction-verified)
records actual private boot/input retests; keep #113 open for publication/review.

## Supported contract

The guest monitor owns protected IDT/TSS entry, interrupt reflection and VM86
return policy. It must enter a synthetic ROM thunk with a normal 16-bit BIOS
FLAGS/CS/IP frame on the VM86 SS:SP stack. The monitor's nine-dword IRETD frame
is a separate structure. The BIOS does not fabricate protected interrupt
reflection or interpret arbitrary monitor frames. Standard VM86 IRET requires
IOPL=3; lower-IOPL returns retain the CPU's #GP path for monitor handling.
The ROM OUT F0 instruction must pass the normal TSS I/O-bitmap check. That trap
requests the synthetic service, whose device operations follow the existing
BIOS policy and controller paths.

| Services | VM86 behavior |
| --- | --- |
| INT 08 | Translated BIOS tick word, EOI after access preflight; expired INT 1C interval tail-transfers to the callback with VM86 CS cache and CPL3 retained. The callback receives the IRQ's BIOS frame and returns with guest IRET. |
| INT 09 / INT 18 keyboard | Translated key-state bitmap/ring pointers/count/modifiers; scan FIFO, BIOS queue and EOI are untouched on access faults. Blocking input executes the ROM STI/HLT/retry sequence, retaining ordinary IOPL and HLT traps for the monitor. |
| Other supported IRQ thunks | Existing master/slave PIC EOI behavior; guest IRET returns using its translated stack. |
| Supported INT 18 display/font services | BIOS work bytes, text/attribute memory and character buffers use guest translations. JIS interfaces retain raw BX:CX addressing; narrow fonts use ES:BX. |
| INT 1B floppy and IDE subset | Saved CF uses translated SS:SP+4 and preserves the remaining saved flags. Disk buffers use guest-linear ES:BP, including noncontiguous physical pages. Floppy sectors still run through FDC FIFO/DMA/scheduler; IDE still uses its PIO task file/data ports. Verify/status operations retain their existing data-access semantics. |
| INT 1C clock/interval | Clock buffers use ES:BX with 16-bit byte-offset wrap; interval tick reset uses the guest work-area mapping. Callbacks can rearm the same existing one-shot timer. |

Each supported request first checks every required memory page with **user**
permissions, even if CR0.WP is clear. No work-area data, buffer data, MMIO read
latch, FIFO or controller is touched during permission checking. Successful
CPU operand writes set page dirty bits; physical floppy DMA does not. Accessed
bits can remain set on faults, as in CPU page walks; denied requests do not dirty
the planned pages. Disk and character/clock buffers retain
their existing distinct linear versus wrapping-offset conventions. Physical
A20, RAM/ROM and device decoding still applies after translation.

The firmware executes inside the OUT instruction's CPU fault boundary. A
missing/supervisor/read-only mapping raises guest #PF with CR2 and P/W/U, saving
the OUT restart EIP. The guest monitor can repair the PTE, invalidate the mapping
and IRETD back to retry once. This preflight avoids partially consumed keyboard
scans, early EOI, discarded timers, partial buffer writes and partially started
disk requests. Saved FLAGS word accesses also check the VM86 SS limit.

The integrated revision additionally supports VM86 Graph LIO GINIT and checked
INT 1F/90 extended copies through the caller's mappings, including valid 386
20-bit limits and page granularity. Unsupported Graph LIO entries and ordinary
protected-mode requests still fail explicitly. This does not add DPMI/general
protected BIOS calls or establish full EMM386/game compatibility. As before,
unsupported services report a diagnostic. Raw device DMA remains physical; only
the currently executing synthetic floppy sector uses
its preflighted guest buffer scatter map.

## Independent reproduction and provenance

[guest/vm86_bios.asm](../guest/vm86_bios.asm) is wholly original MIT assembly,
built offline with NASM. The [pinned manifest](../fixtures/vm86-bios-manifest.json)
records source, license, boot/image hashes, geometry, results and slot budget.
Preparation copies the assembly, root MIT license and manifest alongside the
standalone bootable HDM in the ignored `fixtures/vm86_bios/` directory. No DOS,
external emulator implementation, proprietary firmware or private media is used.
The fixture is test-only and excluded from `web/`.

```sh
python3 tools/prepare_vm86_bios_fixture.py
cargo test --locked --test vm86_bios
npm run build
npx playwright test tests/browser/vm86-bios.spec.js --workers=2
```

The guest starts with identity paging and an actual PIT IRQ0, so its **first**
VM86 BIOS trap is the timer thunk. Its own monitor subsequently reflects IRQ1
and INT 18/1B/1C, builds the BIOS word frame, repairs faults and returns to VM86.
It repeats the sequence with independently mapped work-area/code/stack/buffer
pages and then PG=0. It checks saved flags, a one-shot callback and blocking
keyboard input in each phase. Its original monitor now validates and handles the
ROM HLT #GP at IOPL3 by resuming the retry sequence; a separate counter at 8008h
accounts for those traps without weakening the four expected page-fault checks.
Four deliberate page faults cover a read-only and a
nonpresent tick work area plus missing and supervisor-only buffer pages. The
fault handlers verify the restart address/error, intact first buffer bytes and
unacknowledged timer IRQ before repair. Native and browser tests inject only an
A key after the guest announces its input wait, require `VM86 BIOS PASS`, compare
result bytes `03 00 04 00`, repeat after reset and verify the disk is unchanged.
Browser tests run the production Rust Wasm at both `/` and `/pc98-rust/` and
send the key through the application keyboard events.

Fail-before evidence: an isolated archive of upstream checkpoint
`4a6d7859f5cb782f776913d47b3af7fcfe7f3fd5`, with this same generated fixture and
native regression test, fails at the paged VM86 timer trap with
`synthetic real-mode firmware requested from protected mode`. The updated core
passes the identical cold/reset guest. The baseline failure uses only the
original public fixture; no private traces or media are part of this evidence.

Additional native controls check exact user #PF error/CR2 and OUT retry,
unconsumed keyboard FIFO/queue/EOI, translated saved flags and SS limits, expired
callback retention/CPL3/IRET, cross-page and segment-wrapping clock buffers,
font-source preflight before definition or MMIO read-latch changes, scatter
floppy read/write and IDE PIO read/write with no controller activity before a
denied final page. They also check trap I/O denial and explicit unsupported
protected/VM86 boundaries. Existing native and browser controls cover real-mode
BIOS behavior and both machine profiles.

## Historical PR #119 validation results

Checks run in the assigned worktree:

| Command | Result |
| --- | --- |
| All 48 Python preparation/check commands in README's reproduction block, plus `python3 tools/prepare_vm86_bios_fixture.py` | Passed; includes seven fixture-fetch unit tests, all original probes, pinned FreeDOS binaries and matching source archives/license copies, font and DOS/HDD/device/application fixtures. |
| Isolated baseline `cargo test --locked --test vm86_bios original_vm86_bios_guest_cold_reset -- --nocapture` | Expected failure at the paged VM86 timer firmware guard. |
| `cargo test --locked --test vm86_bios` | 11 passed, including cold/reset of the independent guest. |
| `cargo test --locked` | 534 passed, zero failed/ignored; includes existing real-mode BIOS controls. |
| `cargo clippy --all-targets -- -D warnings` | Passed. |
| `cargo fmt --check`; `git diff --check` | Passed. |
| `npm ci` | Passed; zero reported vulnerabilities. |
| `npx playwright install chromium --only-shell` | Passed; pinned Chromium headless shell available. The full Chromium download stalled after transfer and was stopped; the installed headless shell runs the real browser checks. |
| `npm run build`; `npm run check:site` | Passed; optimized release `wasm32-unknown-unknown` library built, production artifact copied to `web/core.wasm`, all 10 approved publication assets audited. |
| `npx playwright test tests/browser/vm86-bios.spec.js --workers=2` | 2 passed; production Wasm, cold/reset and keyboard events at both URL roots. |
| `npx playwright test --config target/issue-118-playwright.config.mjs` | 164 passed in 7.0 minutes, zero failures; complete production-Wasm Chromium suite at `/` and `/pc98-rust/`, including the new cold/reset VM86 BIOS guest. |

The initial `npm test -- --workers=2` rebuilt/audited the production artifact
successfully, then stopped because another local server occupied port 8000.
The complete browser run uses an ignored `target/issue-118-playwright.config.mjs`
which imports the repository config, preserves its tests/timeouts/projects,
sets two workers and substitutes ports 18800/18801. The first server serves
`web/` at `/`; the second serves a temporary `pc98-rust -> web/` symlink at
`/pc98-rust/`, preserving root-asset rejection. This avoids changing or stopping
other worktrees' servers. No deployed-site checks or private playthrough were
performed; no post-merge verification is required.
