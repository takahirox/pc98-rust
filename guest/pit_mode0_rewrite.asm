; Original binary mode-0 count-only rewrite/IRQ0 probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pit_mode0_rewrite.asm -o pit_mode0_rewrite.bin
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
mov byte [es:0x8001], 0          ; total hardware IRQ0 entries
mov word [es:8*4], timer
mov ax, cs
mov [es:8*4+2], ax
mov al, 0xff
out 2, al
out 0x0a, al
mov al, 0x30                    ; lower OUT before initializing PIC
out 0x77, al
; Discard any firmware/setup request, before starting the count-only lifecycles.
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1                       ; normal EOI, no AEOI
out 2, al
mov al, 0xfe
out 2, al
; Mode 0 was programmed ONCE above. Start a long active countdown.
mov ax, 60000
out 0x71, al
mov al, ah
out 0x71, al
mov cx, 500                     ; old count is still active after this delay
.active:
loop .active
mov bp, 1
.arm:
cli
mov ax, 2000
out 0x71, al                    ; first byte must stop count and lower OUT
sti
mov cx, 40000                   ; >65536 PIT clocks, longer than old countdown
.partial:
loop .partial                   ; no timer reads between the two write bytes
cli
mov ax, bp
dec ax
cmp [es:0x8001], al              ; no IRQ from old count or held-high OUT
jne fail
mov al, 0x0a
out 0, al
in al, 0
and al, 1
jnz fail
mov al, 0x0b
out 0, al
in al, 0
and al, 1
jnz fail
mov bx, bp
mov byte [es:bx+0x8009], 1       ; delayed pair proof at 800ah/800bh
mov al, 2000 >> 8
out 0x71, al                    ; completes replacement, no control word
sti
mov cx, 6000                    ; bounded wait: never HLT awaiting a lost IRQ
.wait_irq:
mov ax, bp
cmp [es:0x8001], al
je .received
loop .wait_irq
jmp fail
.received:
call read_count
cmp ax, 60000                   ; must already have wrapped past terminal zero
jb fail
cmp ax, 65535
jae fail
mov bx, ax
mov cx, 500
.read_delay:
loop .read_delay
call read_count
cmp ax, 60000
jb fail
sub bx, ax                      ; counter advances after terminal, once/clock
cmp bx, 900
jb fail
cmp bx, 1100
ja fail
mov bx, bp
mov byte [es:bx+0x8005], 1       ; per-arm post-terminal count proof
mov cx, 40000                   ; >65536 PIT clocks: observe a further full wrap
.quiet:
loop .quiet
cli
mov ax, bp
cmp [es:0x8001], al              ; exactly ONE hardware entry per arming
jne fail
mov al, 0x0a
out 0, al
in al, 0
and al, 1
jnz fail                       ; no extra latched IRQ0, even across wrap
mov al, 0x0b
out 0, al
in al, 0
and al, 1
jnz fail
mov bx, bp
mov byte [es:bx+0x8007], 1       ; quiet/wrap proof
inc bp
cmp bp, 3
jb .arm
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
inc byte [es:0x8001]
mov ax, bp
cmp [es:0x8001], al              ; any extra IRQ, including during quiet, fails
jne fail
mov bx, bp
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8001], al           ; ISR entry snapshots at 8002h,8003h
cmp al, 1
jne fail
mov al, 0x20
out 0, al
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8003], al           ; post-EOI at 8004h,8005h
cmp al, 0
jne fail
pop bx
pop ax
iret

read_count:
mov al, 0                       ; ordinary latch; counter must keep running
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
pass_text: db 'PIT MODE0 REWRITE PASS', 0
fail_text: db 'PIT MODE0 REWRITE FAIL', 0
expected:
db 0xa5,2,1,1,0,0,1,1,1,1,1,1
times 20 db 0xa5
times 1024-($-$$) db 0
