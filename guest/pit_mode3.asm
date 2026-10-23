; Original binary mode-3 hardware IRQ0 probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pit_mode3.asm -o pit_mode3.bin
; One 1024-byte IPL sector loaded at 1FC0:0000. Guest owns 8000h..801Fh.
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
mov byte [es:0x8002], 0          ; periodic IRQ0 count
mov word [es:8*4], timer
mov ax, cs
mov [es:8*4+2], ax
mov al, 0xff
out 2, al
out 0x0a, al
mov al, 0x36                   ; channel 0, LSB/MSB, mode 3, binary
out 0x77, al
mov ax, 60000
out 0x71, al
mov al, ah
out 0x71, al
mov cx, 500                    ; well inside the first long period
.delay:
loop .delay
call read_count
cmp ax, 55000
jb fail
cmp ax, 60000
jae fail
test ax, 1                     ; even divisor exposes only even counts
jnz fail
mov bx, ax
mov cx, 500                    ; about 1000 PIT clocks, still in high half
.count_delay:
loop .count_delay
call read_count
cmp ax, 55000
jb fail
test ax, 1
jnz fail
sub bx, ax                     ; subtract TWO per PIT clock, not baseline's one
cmp bx, 1900
jb fail
cmp bx, 2200
ja fail
mov byte [es:0x8001], 1         ; latched subtract-two proof
mov al, 0x3e                   ; new mode word, alias 7, separate initial load
out 0x77, al
mov ax, 2000
out 0x71, al
mov al, ah
out 0x71, al
; Reinitialize PIC AFTER programming and before the first terminal count.
; Any programming-induced rise is discarded here, never counted as periodic.
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1                      ; normal EOI, no AEOI
out 2, al
mov al, 0xfe
out 2, al
.wait_irq:
sti
hlt
cmp byte [es:0x8002], 2
jb .wait_irq
cli
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

timer:
push ax
push bx
inc byte [es:0x8002]
xor bx, bx
mov bl, [es:0x8002]
cmp bl, 2
ja fail
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8002], al          ; ISR entry snapshots at 8003h,8004h
cmp al, 1
jne fail
call read_count
test ax, 1                     ; binary mode 3, even divisor
jnz fail
cmp ax, 2000                   ; periodic count stays within new divisor
ja fail
cmp ax, 0
je fail
mov byte [es:0x8007], 1         ; periodic mode-3 count proof
cmp bl, 2
jne .eoi
mov al, 0xff                   ; stop further delivery after exactly two entries
out 2, al
.eoi:
mov al, 0x20
out 0, al
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8004], al          ; post-EOI snapshots at 8005h,8006h
cmp al, 0
jne fail
pop bx
pop ax
iret

read_count:
mov al, 0                      ; ordinary count latch, not mode programming
out 0x77, al
in al, 0x71
mov ah, al
in al, 0x71
xchg al, ah
ret
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
pass_text: db 'PIT MODE3 PASS', 0
fail_text: db 'PIT MODE3 FAIL', 0
expected:
db 0xa5,1,2,1,1,0,0,1
times 24 db 0xa5
times 1024-($-$$) db 0
