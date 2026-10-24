; Original software-triggered mode-4 strobe/IRQ0 probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pit_mode4.asm -o pit_mode4.bin
; One 1024-byte IPL sector at 1FC0:0000. Results/guards at 8000h..801Fh.
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
mov word [es:8*4], timer
mov ax, cs
mov [es:8*4+2], ax
mov al, 0xff
out 2, al
out 0x0a, al
mov al, 0x30
out 0x77, al                    ; known low OUT
mov al, 0x38
mov dx, 0x3fdf
out dx, al                      ; mode 4 raises OUT: setup, not a strobe
; Discard firmware and the setup request before either measured arming.
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
mov bp, 1
.arm:
cli
; Second arming is count-only; neither arm needs another mode word.
call data_port
mov ax, 60000
out dx, al
mov al, ah
out dx, al
mov cx, 100
.initial_delay:
loop .initial_delay
mov al, 0xd0                    ; replacement LSB of 2000; old count continues
out dx, al
mov cx, 100
.partial_delay:
loop .partial_delay
call read_count                 ; also interleaves reads with unfinished write
cmp ax, 58000
jb fail
cmp ax, 60000
jae fail
mov bx, bp
mov byte [es:bx+0x8009], 1       ; old count survives first replacement byte
call data_port
xor dx, 0x3fa8                  ; switch 0071h <-> 3FD9h across paired write
mov al, 7
out dx, al                      ; completed replacement triggers next-clock load
call read_count
cmp ax, 1900
jb fail
cmp ax, 2000
ja fail
mov bx, bp
mov byte [es:bx+0x800b], 1       ; replacement is active, without fresh control
sti
mov cx, 6000
.wait_irq:
mov ax, bp
cmp [es:0x8001], al
je .received
loop .wait_irq
jmp fail
.received:
call read_count
cmp ax, 60000
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
sub bx, ax
cmp bx, 900
jb fail
cmp bx, 1100
ja fail
mov bx, bp
mov byte [es:bx+0x8005], 1       ; advancing latched counts after the strobe
mov cx, 40000                   ; >65536 PIT clocks, with IF enabled
.quiet:
loop .quiet
cli
mov ax, bp
cmp [es:0x8001], al
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
mov byte [es:bx+0x8007], 1       ; one strobe only, even through counter wrap
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
cmp [es:0x8001], al
jne fail
mov bx, bp
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8001], al
cmp al, 1
jne fail
mov al, 0x20
out 0, al
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x8003], al
cmp al, 0
jne fail
pop bx
pop ax
iret

data_port:
mov dx, 0x71
cmp bp, 1
je .done
mov dx, 0x3fd9
.done:
ret
read_count:
call data_port
add dx, 6
mov al, 0
out dx, al                      ; ordinary latch through one bank
sub dx, 6
in al, dx
mov ah, al
xor dx, 0x3fa8
in al, dx                       ; finish paired read through the other bank
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
pass_text: db 'PIT MODE4 PASS', 0
fail_text: db 'PIT MODE4 FAIL', 0
expected:
db 0xa5,2,1,1,0,0,1,1,1,1,1,1,1,1
times 18 db 0xa5
times 1024-($-$$) db 0
