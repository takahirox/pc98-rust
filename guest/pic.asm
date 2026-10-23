; Original OCW2 hardware-IRQ boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pic.asm -o pic.bin
; Synthetic IPL loads one 1024-byte sector at 1FC0:0000. No software INTs.
bits 16
cpu 8086
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov di, 0x8000
mov cx, 32
mov al, 0xa5
rep stosb
mov byte [es:0x8001], 0
mov byte [es:0x8002], 0
mov word [es:8*4], timer
mov word [es:9*4], keyboard
mov ax, cs
mov [es:8*4+2], ax
mov [es:9*4+2], ax
; Reinitialize the master through its existing cascaded ICW path (AEOI off).
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1
out 2, al
mov al, 0xfc
out 2, al
mov al, 0xff
out 0x0a, al
mov al, 0xc0                    ; IR0 lowest, keyboard above timer
out 0, al
.round:
; One-shot PIT channel 0; expiration latches IR0 while IF remains clear.
mov al, 0x30
out 0x77, al
mov al, 0xe8                    ; divisor 1000
out 0x71, al
mov al, 3
out 0x71, al
inc byte [es:0x8001]            ; host may now inject one Return transition
.wait_competing:
mov al, 0x0a
out 0, al
in al, 0
and al, 3
cmp al, 3
jne .wait_competing             ; guest confirms BOTH requests before STI
xor bx, bx
mov bl, [es:0x8001]
mov [es:bx+0x800f], al          ; three independent IRR=03 snapshots
mov cl, bl
shl cl, 1
sti
nop
.wait_delivery:
cmp [es:0x8002], cl
jb .wait_delivery
jne fail
cli
cmp byte [es:0x8001], 3
jb .round
inc byte [es:0x8001]
; Guest itself checks the complete trace, status snapshots and all guards.
mov si, expected
mov di, 0x8000
mov cx, 32
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
mov si, pass_text
call report
hlt
jmp $

keyboard:
; Drain first, before another bus tick can re-latch the FIFO request. This
; preserves the emulator's existing request/ack abstraction. Main permits AX
; clobber; BX and CX must survive each handler.
in al, 0x41
push bx
xor bx, bx
mov bl, [es:0x8001]
mov [es:bx+0x8003], al
mov ah, 0x61                   ; ordinary specific EOI does not rotate
cmp bl, 1
jne .log
mov ah, 0xa0                   ; round 1: keyboard becomes lowest
.log:
mov al, 1
jmp log_irq
timer:
push bx
mov ah, 0x20                   ; ordinary nonspecific EOI does not rotate
cmp byte [es:0x8001], 2
jne .log
mov ah, 0xe0                   ; round 2: timer becomes lowest
.log:
xor al, al
log_irq:
xor bx, bx
mov bl, [es:0x8002]
cmp bl, 6
jae fail
mov [es:bx+0x8008], al
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8014], al
inc byte [es:0x8002]
mov al, ah
out 0, al
pop bx
iret

fail:
cli
mov si, fail_text
call report
hlt
jmp fail
report:
mov ax, 0xa000
mov es, ax
xor di, di
.print:
lodsb
or al, al
jz .done
xor ah, ah
stosw
jmp .print
.done:
ret
pass_text: db 'PIC PASS', 0
fail_text: db 'PIC FAIL', 0
expected:
db 0xa5,4,6,0xa5,0x1c,0x9c,0x1c,0xa5
; Programmed priority; rotate-nonspecific EOI; rotate-specific EOI.
db 1,0,0,1,1,0,0xa5,0xa5
db 3,3,3,0xa5,2,1,1,2,2,1,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5
times 1024-($-$$) db 0
