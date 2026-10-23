; Original Ap real-mode SMSW boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/smsw.asm -o smsw.bin
; Synthetic IPL loads this single 1024-byte sector at 1FC0:0000.
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
mov cx, 80
mov al, 0xa5
rep stosb
mov word [es:6*4], ud
mov word [es:13*4], gp
mov ax, cs
mov [es:6*4+2], ax
mov [es:13*4+2], ax
mov byte [es:0x8049], 0
mov byte [es:0x804a], 0
; Both register widths preserve the nonzero upper half. SMSW preserves flags.
mov eax, 0xabcdffff
push word 0x8d7
popf
smsw ax
pushf
pop dx
cmp dx, 0x8d7
jne fail
cmp eax, 0xabcd0010
jne fail
mov [es:0x8001], eax
mov ecx, 0xbcdeffff
o32 smsw ecx
cmp ecx, 0xbcde0010
jne fail
mov [es:0x8009], ecx
cld
smsw [es:0x8011]
o32 smsw [es:0x8019]
mov ebp, 0x8020
mov esi, 1
; Default SS, SIB, repeated overrides are deliberately encoded.
db 0x66, 0x66, 0x67, 0x67, 0x0f, 0x01, 0x64, 0x35, 0
; Last valid word starts at FFFE; FFFF must not partially replace it.
mov byte [es:0xfffd], 0xa5
smsw [es:0xfffe]
mov bx, 0xffff
mov word [expected_ip], ds_fault
push word 2
popf
ds_fault:
db 0x66, 0x26, 0x0f, 0x01, 0x27 ; o32 smsw [es:bx]
mov ebp, 0x10000
mov word [expected_ip], ss_fault
push word 2
popf
ss_fault:
db 0x66, 0x67, 0x0f, 0x01, 0x65, 0 ; o32 smsw [ss:ebp]
; #UD handlers repair LOCK itself, IRET to the first prefix, and retry.
mov bx, 0x8039
mov word [expected_ip], lock_mem
mov word [repair_ip], lock_mem+1
push word 2
popf
lock_mem:
db 0x26, 0xf0, 0x66, 0x0f, 0x01, 0x27
mov eax, 0xcdefffff
mov word [expected_ip], lock_reg
mov word [repair_ip], lock_reg+1
push word 2
popf
lock_reg:
db 0x66, 0xf0, 0x0f, 0x01, 0xe0
cmp eax, 0xcdef0010
jne fail
mov [es:0x8041], eax
cmp byte [es:0x8049], 2
jne fail
cmp byte [es:0x804a], 2
jne fail
cmp word [es:0xfffe], 0x10
jne fail
cmp byte [es:0xfffd], 0xa5
jne fail
; Every memory result and every untouched byte, including o32 guards.
mov di, 0x8011
.check_words:
cmp word [es:di], 0x10
jne fail
add di, 8
cmp di, 0x8041
jb .check_words
mov si, guards
mov cx, guards_end-guards
.check_guards:
lodsb
xor ah, ah
mov di, ax
cmp byte [es:di+0x8000], 0xa5
jne fail
loop .check_guards
mov si, pass_text
call report
cli
hlt
jmp $

%macro frame 0
push bp
mov bp, sp
push ax
cmp bp, 0x7bf8                  ; exactly six bytes before our push BP
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
; No partial write at FFFF and no wrap to zero on SS:10000.
cmp word [es:0xfffe], 0x10
jne fail
inc byte [es:0x8049]
cmp byte [es:0x8049], 1
je .ds
cmp byte [es:0x8049], 2
jne fail
mov word [ss:bp], 0x8031
and ebp, 0xffff                 ; repair EBP high half before restoring low BP
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
mov byte [cs:si], 0x26         ; same-length ES override replaces LOCK
inc byte [es:0x804a]
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
pass_text: db 'SMSW PASS', 0
fail_text: db 'SMSW FAIL', 0
guards: db 0,5,6,7,8,13,14,15,16,19,20,21,22,23,24,27,28,29,30,31,32
        db 35,36,37,38,39,40,43,44,45,46,47,48,51,52,53,54,55,56
        db 59,60,61,62,63,64,69,70,71,72,75,76,77,78,79
guards_end:
times 1024-($-$$) db 0
