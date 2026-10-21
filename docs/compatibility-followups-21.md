# Verified follow-up Issues — Issue #21

These are **three separate, published tasks**, each carrying the `task` label:
[#23](https://github.com/takahirox/pc98-rust/issues/23) (drawing timing mode),
[#24](https://github.com/takahirox/pc98-rust/issues/24) (Graph LIO/GINIT), and
[#25](https://github.com/takahirox/pc98-rust/issues/25) (held-key bitmap).
The [research report](compatibility-investigation-21.md) contains the
six-title matrix, acquisition identifiers, configuration, isolation and evidence.
The three contracts below have original, game-free reproductions; remaining
unexplained video/input/audio behavior has no speculative ticket.

Repository Issues, open and closed, were checked on 2026-10-21. Existing #14–#16
cover different BIOS/GDC contracts; the older general compatibility tasks are not
dedicated duplicates. The inventory was rechecked before filing these tasks on
2026-10-21. Each published Issue includes its original probe and preparation
steps, tested commit/profile, lawful source/version/hash and acceptance criteria.

All probe source below was newly written for this investigation and is offered
under this repository's MIT license. It contains no game instructions, art,
audio, font, DOS or ROM bytes. Each exact source was assembled locally with NASM
and executed using the unchanged Rust core at main
`0adef0c2c6227506a2a6b447eb80a4df297ab9c7` (file-identical assigned checkpoint
`997a281b5cd1c66303ed1590baaa078744d83c96`). These are failing baseline controls,
not implemented fixes. No existing fixture, mandatory CI or publication behavior
has been changed to run them.

## Shared local probe preparation

Save one source block as its indicated `.asm` file **outside the worktree** and
assemble it with `nasm -f bin probe.asm -o probe.com`. Use the repository's existing
checksum-pinned FreeDOS preparation with `DEST` redirected outside the worktree,
keeping its matching complete source archives and notices there. Extract
`KERNEL.SYS`, `COMMAND.COM`, `FDCONFIG.SYS` and `COPYING` with `extract_fat_file`;
use `image_with_files` to add the original COM and an AUTOEXEC invoking it followed
by `echo PROBE SHELL RETURN`. Use the original boot sector and writable HDM media.
Do not patch DOS or acquire games for these controls.

Run `cargo build --locked --release --bin headless`, then
`target/release/headless /absolute/private/probe.hdm 16000000 --ap`.
Keyboard injection and reset-entry observations use the same public Rust
`Machine` API as the existing native tests; their exact schedules are below.
Source hashes identify the executed UTF-8/LF files, including their final newline.

## A. Support BIOS INT 18h/AH=4Ah drawing timing mode used by PIPOO

### Problem

PIPOO 1.00 now passes its user-font call after #16, but its subsequent valid
`INT 18h` with AX=4A00h and CX=0600h stops with
`unsupported BIOS INT 18 AH=4a`. It returns to `2ff9:0055` if serviced; the tested
core errors in the ROM thunk instead, at slot 26,574,515. This is independent of
the font defect and requires no PIPOO file to reproduce.

### Expected outcome

Implement the documented drawing-timing selection in original Rust for the Ap
DOS profile. CH=06h requests flash drawing and CH=16h flashless drawing; preserve
all guest registers. Express the mode in the emulated GDC/BIOS state consistently
with the current deterministic scheduler and explicitly document timing limits.
Successful return alone must not be presented as PIPOO playability.

### Context and reproduction

- Specification: [NEC BIOS databook, printed p.91, PDF index 107](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf).
- Actual guest: [author's PIPOO release](https://www.vector.co.jp/soft/dos/game/se005985.html),
  exact 1.00 archive/hash and legal private FreeDOS setup in the report.
- Execute original `drawmode.asm` below on the pinned FreeDOS Ap setup. At
  4,393,206 slots, the headless process exits 1 with the same unsupported-BIOS
  error; `DRAWMODE RETURNED` is not reached.
- Executed source SHA-256:
  `9c4e0fb1d0a043e5cc7b0978b53be2be89431df6a5d74af23637c5630bfdc792`.

### Pre-merge acceptance criteria

- [ ] The original control returns to DOS without a core error on the Ap profile.
- [ ] Native tests exercise both valid CH values, verify register preservation
  and relevant GDC/BIOS state, repeated switching and reset behavior. Unsupported
  subsets and VM behavior have an explicit, justified contract.
- [ ] A redistributable guest control verifies the same return/state through real
  FreeDOS and the production Wasm core, with bounded automatic assertions.
- [ ] Run repository-required native/build/browser checks; preserve existing
  fixture hashes and publication assets. No game, proprietary ROM or font is a
  CI download, fixture or acceptance requirement.

### Optional validation

Re-run legally held PIPOO locally and report subsequent stops separately.

### Required post-merge verification

None.

### Original `drawmode.asm`

```asm
; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov ax,4a00h
mov cx,0600h
int 18h
mov dx,returned
mov ah,9
int 21h
mov ax,4c00h
int 21h
returned: db 'DRAWMODE RETURNED',13,10,'$'
```

## B. Provide an original Graph LIO ROM directory and GINIT service

### Problem

TRIANS98 1.02's formerly unexplained boot loop now has a positively observed
immediate cause. Its runtime registers LIO vectors from F990h's ROM table, then
invokes GINIT through INT A0h. The missing table's zero offset points to F990:0000;
execution traverses zero-filled ROM to physical FFFF0h and runs the synthetic
reset jump. It does not reach the writable user-file prompt. Original
`lioinit.asm` reproduces this without game assets or a proprietary ROM.

### Expected outcome

Provide an original Rust/synthetic-ROM entry directory compatible with the
documented F990h interface and implement the evidenced GINIT contract. Do not
copy NEC ROM code or import an emulator. GINIT must initialize the specified
work/display state, return the documented status and preserve DS/SS/SP. Scope
later LIO functions to independently evidenced needs; this task does not assert
that GINIT alone makes TRIANS98 playable or require every LIO function.

### Context and reproduction

- Specification: [NEC BIOS databook, printed pp.138 and 141–143](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf):
  ROM directory/vector registration, work/stack and GINIT contracts.
- Actual guest: [Toxsoft's TRIANS98 release](https://www.vector.co.jp/soft/dos/game/se011295.html),
  exact 1.02 archive/hash and unmodified FreeDOS setup in the report.
- Actual guest provenance: A0h vector becomes F990:0000 at slots
  5,383,378–5,383,379; `INT A0h` at `2d90:79e3` executes at 5,383,474. The saved
  frame returns to `2d90:79e5`, SS:SP=`3ee8:8ade`. The first reset-vector alias
  occurs at 5,396,651; firmware entry follows one slot later.
- The original probe saves the previous DOS vector, reads the GINIT offset at
  F990:0006, registers it, provides a 1200h-byte work area and sufficient DOS
  stack, then calls it. A working service would print `LIO RETURNED`, restore
  the vector and exit. On baseline it prints `LIO ENTER` and reboots instead:
  first F990:66F0 entry at 4,417,109; three reset entries including initial boot
  in 10M slots. No host reset or guest code patch is used.
- Executed source SHA-256:
  `53a6da02462395a8b5a56a9be33c7ab9f2f3278a38625e87a4344724c9ad23a3`.

### Pre-merge acceptance criteria

- [ ] Original control reaches `LIO RETURNED` and `PROBE SHELL RETURN` within a
  bounded run, returns AH=00h and enters the reset vector only for initial boot.
- [ ] Verify directory layout/entries against the published contract, including
  guest registration from ROM rather than only a host interception of INT A0h.
- [ ] Native and redistributable guest tests verify GINIT's prescribed display,
  palette/page/work-area state, DS/SS/SP preservation, guards, repeated calls and
  reset. Bound or explicitly reject unsupported subsets; do not silently pretend
  to implement drawing commands.
- [ ] Exercise the legal control through real DOS on native and production Wasm;
  run the repository's required checks. Add no copied ROM, game or proprietary
  asset to fixtures, downloads or publication.

### Optional validation

Re-test legally held TRIANS98 locally to identify later initialization needs.
No claim about later drawing, user-file creation, gameplay or save/reload follows
from this reset diagnosis.

### Required post-merge verification

None.

### Original `lioinit.asm`

```asm
; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov ax,35a0h
int 21h
mov [oldoff],bx
mov [oldseg],es
mov ax,0f990h
mov ds,ax
mov dx,[6] ; GINIT offset in the ROM entry table
mov ax,25a0h
int 21h
push cs
pop ds
mov dx,entering
mov ah,9
int 21h
mov ax,cs
add ax,(work-$$+100h)/16
mov ds,ax
int 0a0h
push cs
pop ds
mov [result],ah
mov dx,returned
mov ah,9
int 21h
mov dx,[oldoff]
mov ax,[oldseg]
mov ds,ax
mov ax,25a0h
int 21h
mov ah,4ch
mov al,[cs:result]
int 21h
oldoff: dw 0
oldseg: dw 0
result: db 0
entering: db 'LIO ENTER',13,10,'$'
returned: db 'LIO RETURNED',13,10,'$'
align 16
work: times 1200h db 0
```

## C. Synchronize the BIOS held-key bitmap used by MARKADIA

### Problem

Host key injection updates `Keyboard::pressed` and BIOS AH=04h sensing, but the
BIOS work-area key bitmap at 0000:052Ah–0539h stays stale. MARKADIA 1.05 directly
polls these bytes; holding Return leaves its read of 052Dh at zero despite
device group 3 being 10h. This is a specific keyboard contract defect, distinct
from the already-fixed GDC horizontal blank and from unexplained graphics/audio.
MUTATE controls also failed to establish interaction, but their cause is not
attributed to this defect without equivalent evidence.

### Expected outcome

Keep the BIOS memory bitmap coherent with serviced make/break input and grouped
BIOS sensing, following the real keyboard/firmware route. Preserve the existing
guest DOS/TSR interrupt path and ring-buffer behavior. Do not inject game variables
or implement title-specific controls.

### Context and reproduction

- [PC-9801 Programmer's Bible, original authors, printed p.69](https://pc98.ne.jp/devdocs/pc-9801programmersbible.pdf)
  documents the work-area key-state table; [NEC BIOS databook pp.32–33](https://pc98.ne.jp/devdocs/pc-9800technicaldatabookbios.pdf)
  specifies grouped state sensing. Group index is scan/8; the pressed bit is
  scan modulo 8. Return scan 1Ch is group 3, bit 4, at 052Dh.
- Actual consumer: [MARKADIA author's release](https://www.vector.co.jp/soft/dos/game/se001354.html),
  1.05, supplied MDRV98 plus pinned FreeDOS/Ap. At 60M slots hold Return for 2M;
  device group 3 is 10h, all 16 memory bitmap bytes remain zero. Bounded operand
  observation identifies 119 reads of 052Dh at `2d6c:300f` in that window and
  reads of several other bitmap bytes. No game instructions need be published.
- Boot original `keymap.asm` for 5M slots; call `m.bus.key(0x1c, true)`, run 1M,
  then release and run another 1M. It prints `KEYMAP BWA STALE` and returns to DOS
  without a core error. BIOS AH=04h already sees the hold; the direct byte does not.
  Expected output is both `KEYMAP DOWN PASS` and `KEYMAP UP PASS` after release.
- Executed source SHA-256:
  `833f8925cf99c2cbbcce5f05fd3d7a1c129ab229276203a7661eb4ad497440f5`.

### Pre-merge acceptance criteria

- [ ] Original control passes hold and release through real DOS, native and
  production Wasm, using ordinary key input and bounded automatic assertions.
- [ ] Native/guest tests cover multiple groups, simultaneous holds, break codes,
  serviced IRQ delivery and repeated presses. Verify bitmap, BIOS sensing and
  keyboard ring agree without unrelated memory writes or a guest-specific shim.
- [ ] Specify reset/initialization behavior from the documented contract and
  preserve existing keyboard, DOS, timer and application acceptance checks.
- [ ] No MARKADIA, game-specific data or proprietary firmware/font is required
  by hosted CI, the fixtures or Pages publication.

### Optional validation

Re-test MARKADIA and the MUTATE controls locally. Report whether input or video
progresses; do not infer correct gameplay/music from a bitmap test alone.

### Required post-merge verification

None.

### Original `keymap.asm`

```asm
; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov dx,ready
mov ah,9
int 21h
xor ax,ax
mov es,ax
wait_down:
mov ax,0403h
int 18h
test ah,10h
jz wait_down
test byte [es:052dh],10h
jz stale
mov dx,downok
mov ah,9
int 21h
wait_up:
mov ax,0403h
int 18h
test ah,10h
jnz wait_up
test byte [es:052dh],10h
jnz stale
mov dx,ok
jmp print
stale: mov dx,bad
print:
mov ah,9
int 21h
mov ax,4c00h
int 21h
ready: db 'KEYMAP READY: hold Return then release',13,10,'$'
downok: db 'KEYMAP DOWN PASS',13,10,'$'
ok: db 'KEYMAP UP PASS',13,10,'$'
bad: db 'KEYMAP BWA STALE',13,10,'$'
```
