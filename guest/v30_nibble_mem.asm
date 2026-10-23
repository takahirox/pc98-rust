; Original V30 memory nibble-rotation probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/v30_nibble_mem.asm -o v30_nibble_mem.bin
; IPL sector at 1FC0:0000; guest-owned results at 8000h..8023h.
bits 16
cpu 186
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov al, 0xff
out 2, al
out 0x0a, al
mov di, 0x8000
mov cx, 36
mov al, 0xa5
rep stosb
mov byte [es:0x9000], 0xa5
mov byte [es:0x9002], 0x5a
; Each record: memory result, masked AL low nibble, AH, guard,
; complete FLAGS readback, saved FLAGS. AL upper nibble is unspecified by NEC.
; Operand at 9001h has immediate guards 9000h=a5 and 9002h=5a.
%macro probe 5
    mov byte [es:0x9001], %3
    mov ax, %4
    push word %5
    popf
    pushf
    pop dx
    db 0x26, 0x0f, %1, 0x06
    dw 0x9001
    pushf
    pop cx
    mov bl, [es:0x9001]
    mov [es:%2], bl
    mov [es:%2+1], al
    mov [es:%2+2], ah
    and byte [es:%2+1], 0x0f
    mov [es:%2+4], cx
    mov [es:%2+6], dx
    cmp cx, dx
    jne fail
    cmp byte [es:0x9000], 0xa5
    jne fail
    cmp byte [es:0x9002], 0x5a
    jne fail
%endmacro
; Distinct input nibbles, including nondecimal A..F, and distinct AL high nibbles.
probe 0x28, 0x8001, 0x12, 0x5a83, 0xfcd7
probe 0x2a, 0x8009, 0x12, 0x6bf3, 0xf002
probe 0x28, 0x8011, 0xaf, 0x7c0e, 0xf002
; Last record uses bytes 8019..8020; guards extend through 8023.
probe 0x2a, 0x8019, 0xaf, 0x8d7e, 0xfcd7
cld
mov si, expected
mov di, 0x8000
mov cx, 36
check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop check
cmp sp, 0x7c00
jne fail
mov si, pass_text
call report
hlt
jmp $
fail:
cli
cld
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
pass_text: db 'V30 NIBBLE MEM PASS', 0
fail_text: db 'V30 NIBBLE MEM FAIL', 0
expected:
db 0xa5
db 0x23, 1, 0x5a, 0xa5
dw 0xfcd7, 0xfcd7
db 0x31, 2, 0x6b, 0xa5
dw 0xf002, 0xf002
db 0xfe, 0x0a, 0x7c, 0xa5
dw 0xf002, 0xf002
db 0xea, 0x0f, 0x8d, 0xa5
dw 0xfcd7, 0xfcd7
times 3 db 0xa5
times 1024-($-$$) db 0
