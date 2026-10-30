; Original real-mode IRET boundary regression. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; Intel486 PRM (1992), IRET real-address operation: successive pops.
bits 16
cpu 486
org 0
cli
cld
xor ax, ax
mov es, ax
mov ss, ax
mov sp, 0x7c00
push cs
pop ds
mov word [es:12*4], fail
mov word [es:13*4], fail
mov ax, cs
mov [es:12*4+2], ax
mov [es:13*4+2], ax
mov byte [es:0x8000], 0xa5
mov byte [es:0x8001], 0
mov byte [es:0x8002], 0xa5
mov ax, 0x3000
mov ss, ax
; Actual software interrupt entry at SP=2 pushes across the boundary.
; The original synthetic cursor BIOS returns through the shared IRET.
mov sp, 2
mov ah, 0x11
stc
int 0x18
jnc fail
cmp sp, 2
jne fail
inc byte [es:0x8001]
; Independently authored word frames, wrapping before CS or before FLAGS.
mov word [ss:0xfffc], word_return
mov ax, cs
mov [ss:0xfffe], ax
mov word [ss:0], 3
mov sp, 0xfffc
iret
word_return:
jnc fail
cmp sp, 2
jne fail
inc byte [es:0x8001]
mov word [ss:0xfffe], word_return2
mov [ss:0], ax
mov word [ss:2], 3
mov sp, 0xfffe
iret
word_return2:
jnc fail
cmp sp, 4
jne fail
inc byte [es:0x8001]
; Operand-size and stack-address-size are independent: each dword fits,
; but the 16-bit SP wraps between the dword slots.
movzx eax, ax
mov dword [ss:0xfff8], dword_return
mov [ss:0xfffc], eax
mov dword [ss:0], 3
mov sp, 0xfff8
iretd
dword_return:
jnc fail
cmp sp, 4
jne fail
inc byte [es:0x8001]
mov dword [ss:0xfffc], dword_return2
mov [ss:0], eax
mov dword [ss:4], 3
mov sp, 0xfffc
iretd
dword_return2:
jnc fail
cmp sp, 8
jne fail
inc byte [es:0x8001]
cmp byte [es:0x8000], 0xa5
jne fail
cmp byte [es:0x8002], 0xa5
jne fail
mov si, pass_text
jmp report
fail:
mov si, fail_text
report:
cli
xor ax, ax
mov ss, ax
mov sp, 0x7c00
push cs
pop ds
mov ax, 0xa000
mov es, ax
xor di, di
.print:
lodsb
or al, al
jz .halt
xor ah, ah
stosw
jmp .print
.halt:
hlt
jmp .halt
pass_text: db 'IRET WRAP PASS', 0
fail_text: db 'IRET WRAP FAIL', 0
times 1024-($-$$) db 0
