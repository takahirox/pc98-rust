; Original OCW3 polling boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pic_poll.asm -o pic_poll.bin
; Synthetic IPL loads one 1024-byte sector at 1FC0:0000.
bits 16
cpu 8086
org 0
%macro snapshot 3
mov al, %2
out %1, al
in al, %1
mov [es:0x8000+%3], al
%endmacro
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
mov byte [es:0x800e], 0
mov word [es:8*4], timer
mov ax, cs
mov [es:8*4+2], ax
; Initialize both controllers, normal EOI, fixed PC-98 cascade.
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x10
out 10, al
mov al, 0x80
out 2, al
mov al, 7
out 10, al
mov al, 1
out 2, al
out 10, al
mov al, 0x7e                 ; master timer and cascade
out 2, al
mov al, 0xf7                 ; slave FDC IR3
out 10, al
call start_timer
.wait_pit:
mov al, 0x0a
out 0, al
in al, 0
 test al, 1
jz .wait_pit
snapshot 0, 0x0c, 1         ; local level 0, not vector 8
snapshot 0, 0x0b, 2         ; ISR0 set
mov al, 0x20
out 0, al
snapshot 0, 0x0b, 3
; Issue real FDC READ ID, drive 0/head 0. No DMA or disk writes.
mov al, 0x4a
out 0x92, al
xor al, al
out 0x92, al
.wait_fdc:
in al, 0x90
and al, 0xd0
cmp al, 0xd0
jne .wait_fdc
.wait_cascade:
mov al, 0x0a
out 0, al
in al, 0
test al, 0x80
jz .wait_cascade
snapshot 0, 0x0c, 4         ; master acknowledges IR7 only
snapshot 8, 0x0b, 5         ; slave ISR still empty
snapshot 0, 0x0b, 6
snapshot 8, 0x0f, 7         ; P overrides combined ISR read
snapshot 8, 0x0b, 8
mov di, 0x8011
mov cx, 7
.read_id:
in al, 0x92
stosb
loop .read_id
mov al, 0x20
out 8, al
snapshot 0, 0x0b, 9         ; slave EOI leaves master ISR7
snapshot 8, 0x0b, 10
mov al, 0x20
out 0, al
snapshot 0, 0x0b, 11
snapshot 0, 0x0c, 12
snapshot 8, 0x0c, 13
; Polls are consumed. Ordinary IRQ0 must still deliver through the CPU.
call start_timer
sti
nop
.wait_irq:
cmp byte [es:0x800e], 1
jb .wait_irq
jne fail
cli
snapshot 0, 0x0b, 16
; Guest verifies the complete result buffer including every guard.
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
start_timer:
mov al, 0x30                 ; mode 0 one-shot, divisor 1000
out 0x77, al
mov al, 0xe8
out 0x71, al
mov al, 3
out 0x71, al
ret
timer:
push ax
inc byte [es:0x800e]
snapshot 0, 0x0b, 15
mov al, 0x20
out 0, al
pop ax
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
pass_text: db 'PIC POLL PASS', 0
fail_text: db 'PIC POLL FAIL', 0
expected:
db 0xa5,0x80,1,0,0x87,0,0x80,0x83,8,0x80,0,0,0,0,1,1
 db 0,0,0,0,0,0,1,3,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5
times 1024-($-$$) db 0
