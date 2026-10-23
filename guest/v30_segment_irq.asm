; Original V30 segment MOV/POP maskable-INT timing probe.
; SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/v30_segment_irq.asm -o v30_segment_irq.bin
; Synthetic IPL loads this one-sector probe at 1FC0:0000.
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
mov cx, 48
mov al, 0xa5
rep stosb
mov word [es:0x8001], 0       ; case number, handler entries
mov byte [es:0x9000], 0xa5
mov byte [es:0x9003], 0x5a
mov ax, cs
mov [es:0x9001], ax          ; odd, guarded memory segment operand
mov word [es:8*4], handler
mov [es:8*4+2], ax
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1
out 2, al
mov al, 0xfe
out 2, al
mov al, 0xff
out 0x0a, al

; Every case begins with a guest-confirmed pending PIT request and IF clear.
; STI protects the first producer; each producer must renew the boundary.
; Labels are explicit so the handler also checks the precise return IP.
call arm
mov bp, resume1
sti
mov ds, ax
inc byte [es:0x8004]
resume1: call finish
call arm
mov bp, resume2
sti
mov ax, ds
inc byte [es:0x8004]
resume2: call finish
call arm
mov bp, resume3
sti
mov ds, [es:0x9001]
inc byte [es:0x8004]
resume3: call finish
call arm
mov bp, resume4
sti
mov [es:0x9001], ds
inc byte [es:0x8004]
resume4: call finish
call arm
push ds
mov bp, resume5
sti
pop ds
inc byte [es:0x8004]
resume5: call finish
call arm
push es
mov bp, resume6
sti
mov ds, ax
mov ax, ss
mov ds, [es:0x9001]
pop es
inc byte [es:0x8004]
resume6: call finish
call arm
xor ax, ax
mov bp, resume7
sti
mov es, ax
inc byte [es:0x8004]
resume7: call finish
call arm
mov bp, resume8
sti
mov ax, cs
inc byte [es:0x8004]
resume8: call finish
cli
cmp sp, 0x7c00
jne fail
mov si, expected
mov di, 0x8000
mov cx, 48
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
mov ax, cs
cmp [es:0x9001], ax
jne fail
cmp byte [es:0x9000], 0xa5
jne fail
cmp byte [es:0x9003], 0x5a
jne fail
mov si, pass_text
call report
hlt
jmp $

arm:
cli
inc byte [es:0x8001]
mov byte [es:0x8004], 0
mov al, 0x30                ; mode 0, one terminal-count request
out 0x77, al
mov al, 0xe8                ; divisor 1000
out 0x71, al
mov al, 3
out 0x71, al
mov cx, 0xffff
.wait:
mov al, 0x0a
out 0, al
in al, 0
and al, 1
jnz .pending
loop .wait
jmp fail
.pending:
mov [es:0x8003], al
mov ax, cs
ret

handler:
; No pushes: inspect the exact six-byte hardware frame before any guest stack use.
mov bx, sp
cmp bx, 0x7bfa
jne fail
cmp [ss:bx], bp
jne fail
mov dx, cs
cmp [ss:bx+2], dx
jne fail
test word [ss:bx+4], 0x200
jz fail
cmp byte [es:0x8004], 1      ; following INC must precede acknowledgement
jne fail
inc byte [es:0x8002]
mov al, [es:0x8001]
cmp al, [es:0x8002]
jne fail
xor bx, bx
mov bl, al
mov byte [es:bx+0x800f], 1
mov al, 0x0b
out 0, al
in al, 0
cmp al, 1
jne fail
mov [es:0x8005], al
mov al, 0x0a
out 0, al
in al, 0
cmp al, 0
jne fail
mov al, 0x20
out 0, al
mov al, 0x0b
out 0, al
in al, 0
cmp al, 0
jne fail
iret

finish:
cli
mov al, [es:0x8001]
cmp al, [es:0x8002]
jne fail
cmp byte [es:0x8004], 1
jne fail
ret
fail:
cli
mov si, fail_text
call report
hlt
jmp $
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
pass_text: db 'V30 SEGMENT IRQ PASS', 0
fail_text: db 'V30 SEGMENT IRQ FAIL', 0
expected:
db 0xa5,8,8,1,1,1
times 10 db 0xa5
times 8 db 1
times 24 db 0xa5
times 1024-($-$$) db 0
