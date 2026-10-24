; Original Ap x87 initialization probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/fpu_init.asm -o fpu_init.bin
; Synthetic IPL loads this one-sector probe at 1FC0:0000.
bits 16
cpu 486
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
mov cx, 64
mov al, 0xa5
rep stosb
mov word [es:6*4], ud
mov word [es:13*4], gp
mov ax, cs
mov [es:6*4+2], ax
mov [es:13*4+2], ax
mov byte [es:0x8039], 0
mov byte [es:0x803a], 0
; FNINIT and register status readback preserve integer flags and EAX high half.
mov eax, 0xabcdffff
push word 0x8d7
popf
fninit
fnstsw ax
pushf
pop dx
cmp dx, 0x8d7
jne fail
cmp eax, 0xabcd0000
jne fail
mov [es:0x8001], eax
cld
fnstcw [es:0x8009]
o32 fnstsw [es:0x8011]
; Last segment override is GS; repeated 66/67, SIB defaults to SS.
xor ax, ax
mov fs, ax
mov gs, ax
mov bx, 0x8019
db 0x26, 0x64, 0x65, 0x66, 0x66, 0xd9, 0x3f
mov ebp, 0x8020
mov esi, 1
db 0x66, 0x66, 0x67, 0x67, 0xdd, 0x7c, 0x35, 0
mov byte [es:0xfffd], 0xa5
fnstcw [es:0xfffe]
mov bx, 0xffff
mov word [expected_ip], ds_fault
push word 2
popf
ds_fault:
db 0x66, 0x26, 0xd9, 0x3f
mov ebp, 0x10000
mov word [expected_ip], ss_fault
push word 2
popf
ss_fault:
db 0x66, 0x67, 0xdd, 0x7d, 0
; Guest repairs LOCK in each of the four implemented forms and retries.
%macro locked 2+
mov word [expected_ip], %1
mov word [repair_ip], %1+1
push word 2
popf
%1:
db 0x66, 0xf0, %2
%endmacro
locked lock_init, 0xdb, 0xe3
mov bx, 0x8009
locked lock_cw, 0x26, 0xd9, 0x3f
mov bx, 0x8011
locked lock_sw, 0x26, 0xdd, 0x3f
mov eax, 0xcdefffff
locked lock_ax, 0xdf, 0xe0
cmp eax, 0xcdef0000
jne fail
mov [es:0x8031], eax
cmp byte [es:0x8039], 2
jne fail
cmp byte [es:0x803a], 4
jne fail
cmp word [es:0xfffe], 0x037f
jne fail
cmp byte [es:0xfffd], 0xa5
jne fail
; Compare every result/guard byte against a literal independent oracle.
mov si, expected
mov di, 0x8000
mov cx, 64
check:
lodsb
cmp [es:di], al
jne fail
inc di
loop check
mov si, pass_text
call report
cli
hlt
jmp $

%macro frame 0
push bp
mov bp, sp
push ax
cmp bp, 0x7bf8
jne fail
mov ax, [cs:expected_ip]
cmp [ss:bp+2], ax
jne fail
mov ax, cs
cmp [ss:bp+4], ax
jne fail
cmp word [ss:bp+6], 2
jne fail
%endmacro
gp:
frame
cmp word [es:0xfffe], 0x037f
jne fail
inc byte [es:0x8039]
cmp byte [es:0x8039], 1
je .ds
cmp byte [es:0x8039], 2
jne fail
mov word [ss:bp], 0x802b
and ebp, 0xffff
jmp .done
.ds:
mov bx, 0x8029
.done:
pop ax
pop bp
iret
ud:
frame
push si
mov si, [cs:repair_ip]
cmp byte [cs:si], 0xf0
jne fail
mov byte [cs:si], 0x26
inc byte [es:0x803a]
pop si
pop ax
pop bp
iret
fail:
cld
mov si, fail_text
call report
cli
hlt
jmp fail
report:
push es
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
pop es
ret
expected_ip: dw 0
repair_ip: dw 0
pass_text: db 'FPU PASS', 0
fail_text: db 'FPU FAIL', 0
expected:
db 0xa5,0,0,0xcd,0xab,0xa5,0xa5,0xa5
db 0xa5,0x7f,3,0xa5,0xa5,0xa5,0xa5,0xa5
db 0xa5,0,0,0xa5,0xa5,0xa5,0xa5,0xa5
db 0xa5,0x7f,3,0xa5,0xa5,0xa5,0xa5,0xa5
db 0xa5,0,0,0xa5,0xa5,0xa5,0xa5,0xa5
db 0xa5,0x7f,3,0,0,0xa5,0xa5,0xa5
db 0xa5,0,0,0xef,0xcd,0xa5,0xa5,0xa5
db 0xa5,2,4,0xa5,0xa5,0xa5,0xa5,0xa5
times 1024-($-$$) db 0
