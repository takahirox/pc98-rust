# Verified follow-up tasks — Issue #13

These three individual English Issues were created with label `task` and no
assignee after checking existing Issues for duplicates:
[#14](https://github.com/takahirox/pc98-rust/issues/14),
[#15](https://github.com/takahirox/pc98-rust/issues/15), and
[#16](https://github.com/takahirox/pc98-rust/issues/16).
The sections below preserve their problem, scope and acceptance; each posted
Issue also includes its relevant original probe and standalone local preparation
instructions. The [final research comment on #13](https://github.com/takahirox/pc98-rust/issues/13#issuecomment-6309507515)
is posted. See the [investigation report](compatibility-investigation-13.md)
for baseline, acquisition terms, exact hashes, matrix and limitations.

Each problem below was observed in a period game and independently reproduced
with original source. This is triage of documented missing subsets, not a claim
that the earlier implementation promised full hardware fidelity. Do not combine
these unrelated roots or create one ticket per affected game. Do not make
restricted game downloads a merge requirement. No implementation is included
in this research checkpoint.

## Task #14: Implement PC-98 BIOS one-shot interval callbacks used by DOS sound drivers

Registered: [Issue #14](https://github.com/takahirox/pc98-rust/issues/14), label `task`.

### Problem

On the post-PR #12 Ap profile, the supplied NL sound TSR in Samurai Shoujoden,
Circuit Angel and Dengeki Butouki TURBO registers `INT 1Ch/AH=02h` and then waits
indefinitely for its callback flag. None reaches its game title. Current
`src/firmware.rs` only clears the word at physical `058Ah`; it ignores the
requested interval and callback address. The independent `timer.asm` control
below prints `TIMER CALLBACK TIMEOUT` through the actual FreeDOS kernel.

### Expected outcome

Implement the documented one-shot PC-98 interval service: `CX` is a count of
10 ms intervals, with zero representing 65,536 intervals; `ES:BX` identifies
the guest interrupt routine. Expiry must enter guest code with the correct
interrupt frame and allow its IRET to resume execution. Keep scheduling and
interrupt behavior bounded and deterministic. No host driver recognition or
application-specific wait bypass.

### Context and reproduction

Baseline: `2eb022f0e03306a42527c51228f309eb7e4cadc3`, native release core,
PC-9821Ap DOS profile, original firmware. Boot the original licensed GR-DOS
disks from [MUTATE's publisher page](https://www.mutate-soft.com/km-plus/) after
the report's geometry-validated FDI-to-HDM conversion. Their normal AUTOEXECs
start NL before each game; no separately supplied DOS or sound driver is needed.
All downloads and logs must remain outside version control and publication.

NL requests `CX=0002h`; callback/flag addresses are `108a:1857`/`108a:2130`
(Samurai), `1089:1857`/`1089:2130` (Circuit Angel), and
`108c:1857`/`108c:2130` (TURBO). Initial requests occur at scheduler slots
1,065,590, 1,068,404 and 1,066,799 respectively. All flags remain zero during
the bounded waits. The first game's repeated wait includes PC `108a:1831`
(opcode `E2h`); stable state persists through 42.4 million slots.

Specification: [manufacturer's PC-9800 BIOS databook, timer section,
printed p.13](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf), also
[PC-9801 programmer's reference, printed p.52](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf).
The latter specifies IRET for the callback. Use specifications, not another
emulator's implementation. Scope the fix to the timer service, its firmware/
scheduler/IRQ integration and tests; successful complete game operation may have
additional blockers. Update the current documentation's misleading description
of AH=02h as only a timer reset when implementing the service.

### Pre-merge acceptance criteria

- [ ] An original MIT guest based on `timer.asm` boots through existing legal
  FreeDOS/FreeCOM and prints `TIMER CALLBACK PASS` within a bounded step budget;
  verify the guest flag changes, interrupt stack and return address, not just text.
- [ ] Specification-level native tests verify one expiry for one request,
  10 ms units, no early callback, `CX=0` semantics, interrupt gating, register
  preservation and reset clearing pending state. State a deliberate contract
  for a subsequent request and test it against the selected BIOS reference.
- [ ] Repeated stepping after expiry does not retrigger an unarmed callback.
  No host execution of guest handlers or title/driver-specific special cases.
- [ ] Run the repository's required native, lint, Wasm and actual-browser checks
  with the existing legal fixtures. Exercise the new original timer fixture in
  native and actual Wasm guest execution. Existing VM/Ap timing, music, input,
  disk, graphics and publication checks pass.
- [ ] Tests need no MUTATE media, proprietary BIOS, commercial DOS, manual user,
  physical hardware or post-merge deployment. Keep all original media local only.

### Optional validation

Rerun the three publisher boot disks locally. Record whether NL progresses and
any later stops; passing the callback probe alone does not prove game support.

### Required post-merge verification

None.

## Task #15: Model GDC horizontal-blank status transitions used by MARKADIA

Registered: [Issue #15](https://github.com/takahirox/pc98-rust/issues/15), label `task`.

### Problem

MARKADIA 1.05 launches its supplied music driver and opening code on the Ap
profile, then waits at `2d6c:515b` reading text GDC status port `0060h` (opcode
`ECh`). Its test of bit `40h` never succeeds. In a 35-million-slot run, this
instruction returns `04h` 6,670,932 times and `24h` 444,479 times; bit 6 is always
zero. Graphics state remains stable and keys do not progress the program.

`Bus::input` currently constructs both GDC status registers from FIFO and vertical
sync only. The independent `hblank.asm` control below reports
`HBLANK TRANSITION TIMEOUT` without any MARKADIA bytes.

### Expected outcome

Make horizontal-blank status bit 6 transition with modeled horizontal display
timing at `0060h` and `00A0h`. Preserve FIFO and vertical-sync status semantics.
Use deterministic elapsed guest clocks and a documented supported display mode;
do not toggle based on read count, recognize the game, or patch its wait loop.
Full GDC drawing, exact physical timing, new PEGC modes and unrelated ports are
outside this fix's scope.

### Context and reproduction

Baseline/environment are in the report. Acquire [MARKADIA's author-submitted
Vector release](https://www.vector.co.jp/soft/dos/game/se001354.html), version
1.05, inspect `MARKADIA.DOC` use conditions, and prepare a local-only FreeDOS
floppy with all unchanged archive members. Run the supplied `MKD.BAT`, including
`MDRV98`, rather than silently omitting its driver. Launch with the existing
native runner and `--ap`. Readiness of the opening artwork/audio is unverified;
the specific status-bit wait is the demonstrated defect.

The report pins the archive and prepared image hashes and bounds the run.
Independent control: boot `hblank.asm` below through the same FreeDOS image.
Expected: both a set and a cleared horizontal-blank bit within the bounded
polls. Actual: first phase times out and returns to DOS.

References: [PC-98 display port definitions](https://www.webtech.co.jp/company/doc/undocumented_mem/io_disp.txt)
(`0060h`/`00A0h`, bit 6) and
[NEC uPD7220 GDC Design Manual](https://ftpmirror.your.org/pub/misc/bitsavers/components/nec/uPD72220_GDC_Design_Manual_Version_3.pdf)
for display timing. No existing emulator implementation is needed.

### Pre-merge acceptance criteria

- [ ] An original MIT guest based on `hblank.asm` prints
  `HBLANK TRANSITION PASS` with a bounded budget on initialized video; add
  coverage for both GDC status ports.
- [ ] Native tests sample multiple horizontal periods and verify both states,
  deterministic period/duty behavior for the documented mode, and unchanged
  FIFO/vertical-sync bits. Advancing time without reading ports must also move
  the signal; repeated reads without elapsed time must not manufacture transitions.
- [ ] Reset/mode initialization produces the documented timing state; the core
  preserves scheduler bounds, VM behavior and both existing display paths.
- [ ] Run native, lint, Wasm, actual-browser and publication checks required by
  the repository. The new original guest also passes on actual browser Wasm.
- [ ] No game downloads/data, screenshots, original BIOS, reference emulator or
  human playtest is required in CI or as a merge condition.

### Optional validation

Recheck MARKADIA's opening and keyboard progression using the local licensed
archive. Report additional failures separately; do not claim broad playability
from removal of this single wait.

### Required post-merge verification

None.

## Task #16: Support PC-98 BIOS user-character loading required by PIPOO

Registered: [Issue #16](https://github.com/takahirox/pc98-rust/issues/16), label `task`.

### Problem

PIPOO 1.00 reaches a user-defined-font request during initialization and stops
the core with `unsupported BIOS INT 18 AH=1a`. At scheduler slot 26,559,171,
the return is `3231:0f65`, registration code `DX=7621h`, and font-buffer pointer
`BX:CX=3662:0018`. The preceding run performs 292 disk reads, 33 writes and
graphics-memory changes. Those observations do not establish gameplay or a
working save system.

The current BIOS match has no AH=1Ah case and the replacement-font mechanism
does not supply mutable guest-defined character storage. The original
`fontload.asm` control below produces the same host error without copying any
game glyphs, font ROM or program instructions.

### Expected outcome

Implement the PC-98 16-dot user-character definition service and its mutable
guest font state for the supported profile. The font pointer is **BX:CX**;
DX is the registration code. Retain licensed replacement Japanese/ANK glyphs.
Guest-defined patterns must be available through the supported display and
character-generator read paths with documented reset semantics. Do not silently
ignore the call or substitute host-drawn game text.

### Context and reproduction

Use [PIPOO's author-submitted release](https://www.vector.co.jp/soft/dos/game/se005985.html),
version 1.00. Check `PIPE.DOC` terms first. Keep its entire archive and extracted
files local; no redistribution or publication is needed. Prepare the report's
FreeDOS floppy and run `PIPE` on the Ap profile. The published documentation
explicitly describes the game's use of user-defined characters. The first run
also generates animation/score data; allow the disk to be writable.

The manufacturer's [PC-9800 BIOS databook, printed p.52](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
specifies AH=1Ah, BX:CX, DX, a 32-byte pattern plus two-byte scratch workspace,
and the relevant user-character code ranges/register-preservation contract.
Use that service contract and hardware font documentation, not copied emulator
code. Scope this task to supported user-character loading/storage/readback/
rendering and tests; no general FEP or proprietary font emulation is requested.

### Pre-merge acceptance criteria

- [ ] The original `fontload.asm` guest boots through legal FreeDOS and returns
  without a core error. Extend it with an original asymmetric glyph and automated
  readback/render assertions so a no-op BIOS handler cannot pass.
- [ ] Verify BX:CX addressing independently of DS/ES, buffer/scratch boundaries,
  supported registration-code endpoints, preserved registers and unaffected
  replacement glyphs. Document/test invalid-code behavior against the selected
  model's actual specification rather than assuming arbitrary codes are valid.
- [ ] Replacing the same user character changes its CG/text/graphics consumers
  consistently. Reset follows the documented mutable-font reset policy without
  altering replacement font assets. No unlicensed ROM/glyph data is introduced.
- [ ] Required native, lint, Wasm, browser and publication checks pass. An original
  glyph is exercised in actual browser Wasm and compared to original test pixels.
- [ ] CI and merge acceptance require no PIPOO data, proprietary BIOS/OS/font,
  screenshot inspection, manual game play or physical hardware.

### Optional validation

Rerun PIPOO locally past its initialization and record any later problems. No
claim is made that this single fix provides complete PIPOO support.

### Required post-merge verification

None.

## Original control reproduction

The following three small sources are original work under the repository's MIT
license. They contain no third-party game/ROM/driver bytes. Save each block to
the stated filename in a private directory **outside** the worktree. The preparation
example downloads only the project's already-pinned GPL guest DOS and matching
sources; it downloads no games or emulator implementations. Keep its output local.

From the repository root, after saving the three sources:

```sh
python3 - /absolute/local-only/probes <<'PY'
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, 'tools')
import prepare_freedos as f
dest = Path(sys.argv[1]).resolve()
assert not dest.is_relative_to(Path.cwd().resolve())
dest.mkdir(parents=True, exist_ok=True)
f.DEST = dest / 'freedos'
f.main()
base = (f.DEST / 'minimal.hdm').read_bytes()
common = {n: f.extract_fat_file(base, n) for n in [
    b'KERNEL  SYS', b'COMMAND COM', b'FDCONFIGSYS', b'COPYING    ']}
for name in ['timer', 'hblank', 'fontload']:
    output = dest / (name + '.com')
    subprocess.run(['nasm', '-f', 'bin', '-o', str(output),
                    str(dest / (name + '.asm'))], check=True)
    files = {**common,
             (name.upper().ljust(8) + 'COM').encode(): output.read_bytes(),
             b'AUTOEXECBAT': ('@echo off\r\n' + name +
                              '\r\necho ISSUE13 PROBE RETURNED\r\n').encode()}
    (dest / (name + '.hdm')).write_bytes(f.image_with_files(base[:1024], files))
PY
cargo build --locked --release --bin headless
target/release/headless /absolute/local-only/probes/timer.hdm 16000000 --ap
target/release/headless /absolute/local-only/probes/hblank.hdm 16000000 --ap
target/release/headless /absolute/local-only/probes/fontload.hdm 16000000 --ap
```

Actually observed baseline results: timer and horizontal-blank probes print
their timeout diagnostics and return to DOS; the headless process itself exits
0. The font probe stops with the unsupported AH=1Ah error; headless exits 1 at
4,393,518 retired instructions. These are not three passing tests. They are small
reproductions for future fixes; stronger acceptance assertions above remain
future work.

### timer.asm

```nasm
; Original MIT investigation probe for Issue #13.
bits 16
org 100h
push cs
pop ds
push cs
pop es
mov bx,callback
mov cx,1
mov ah,2
int 1ch
sti
mov cx,0ffffh
.wait:
cmp byte [fired],0
jne passed
loop .wait
mov dx,failed
mov ax,0900h
int 21h
mov ax,4c01h
int 21h
passed:
mov dx,ok
mov ax,0900h
int 21h
mov ax,4c00h
int 21h
callback:
inc byte [cs:fired]
iret
fired: db 0
failed: db 'TIMER CALLBACK TIMEOUT',13,10,'$'
ok: db 'TIMER CALLBACK PASS',13,10,'$'
```

### hblank.asm

```nasm
; Original MIT investigation probe for Issue #13.
bits 16
org 100h
push cs
pop ds
mov dx,60h
mov cx,0ffffh
.high:
in al,dx
test al,40h
jnz seen_high
loop .high
jmp failed
seen_high:
mov cx,0ffffh
.low:
in al,dx
test al,40h
jz passed
loop .low
failed:
mov dx,bad
mov ax,0900h
int 21h
mov ax,4c01h
int 21h
passed:
mov dx,ok
mov ax,0900h
int 21h
mov ax,4c00h
int 21h
bad: db 'HBLANK TRANSITION TIMEOUT',13,10,'$'
ok: db 'HBLANK TRANSITION PASS',13,10,'$'
```

### fontload.asm

```nasm
; Original MIT investigation probe for Issue #13.
bits 16
org 100h
push cs
pop bx
mov cx,pattern
mov dx,7621h
mov ah,1ah
int 18h
mov ax,4c00h
int 21h
pattern:
times 16 dw 08181h
dw 0 ; BIOS scratch workspace
```
