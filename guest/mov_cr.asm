; Original Ap i486 control-register read probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/mov_cr.asm -o mov_cr.bin
; Synthetic IPL loads one 1024-byte sector at 1FC0:0000. No DOS/ROM required.
bits 16
cpu 486
org 0
cli
cld
xor ax, ax
mov ss, ax
mov esp, 0x7c00
mov es, ax
push cs
pop ds
mov di, 0x8000
mov cx, 128
mov al, 0xa5
rep stosb
mov word [es:6*4], ud
mov ax, cs
mov [es:6*4+2], ax
mov byte [es:0x8061], 0

%macro probe 4
mov %1, 0xdeadbeef
push word 0x8d7
popf
%if %4 = 1
    db 0x66, 0x66, 0x67, 0x67, 0x64, 0x65
%elif %4 = 2
    db 0xf2
%elif %4 = 3
    db 0xf3
%endif
db 0x0f, 0x20, 0xc0+%2
mov [es:0x8000+%3], %1
%if %2 = 4
    mov esp, 0x7c00             ; no stack access until ESP is restored
%endif
pushf
pop word [es:0x8059]
cmp word [es:0x8059], 0x8d7
jne fail
cmp dword [es:0x8000+%3], 0x60000010
jne fail
%endmacro
probe eax, 0, 1, 0
probe ecx, 1, 9, 1
probe edx, 2, 17, 2
probe ebx, 3, 25, 3
probe esp, 4, 33, 1
probe ebp, 5, 41, 0
probe esi, 6, 49, 1
probe edi, 7, 57, 0
cld
mov eax, 0xdeadbeef
mov eax, cr2
mov [es:0x8041], eax
test eax, eax
jnz fail
mov eax, 0xdeadbeef
mov eax, cr3
mov [es:0x8049], eax
test eax, eax
jnz fail
smsw [es:0x8051]
cmp word [es:0x8051], 0x10
jne fail

; Each #UD handler checks a six-byte first-prefix frame and repairs the byte;
; IRET retries the same instruction. Memory form has no operand access.
%macro invalid 3
mov word [expected_ip], %1
mov word [repair_ip], %1+%2
mov byte [replacement], %3
push word 2
popf
%endmacro
invalid locked, 1, 0x65
locked:
db 0x66,0xf0,0x0f,0x20,0xc0
cmp eax, 0x60000010
jne fail
invalid reserved, 4, 0xc0
reserved:
db 0x67,0x65,0x0f,0x20,0xe0  ; CR4 is unavailable on the selected i486
cmp eax, 0x60000010
jne fail
invalid memory, 4, 0xc0
memory:
db 0x66,0x26,0x0f,0x20,0x00
cmp eax, 0x60000010
jne fail
cmp byte [es:0x8061], 3
jne fail
; Check all result/guard bytes against an independent literal guest table.
cld
mov si, expected
mov di, 0x8000
mov cx, 128
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
mov si, pass_text
call report
cli
hlt
jmp $
ud:
push bp
mov bp, sp
push ax
push si
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
pushf
pop ax
test ax, 0x300
jnz fail
mov si, [cs:repair_ip]
mov al, [cs:replacement]
mov [cs:si], al
inc byte [es:0x8061]
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
expected_ip: dw 0
repair_ip: dw 0
replacement: db 0
pass_text: db 'MOV CR PASS',0
fail_text: db 'MOV CR FAIL',0
expected:
%rep 8
    db 0xa5
    dd 0x60000010
    times 3 db 0xa5
%endrep
    db 0xa5
    dd 0
    times 3 db 0xa5
    db 0xa5
    dd 0
    times 3 db 0xa5
    db 0xa5
    dw 0x10
    times 5 db 0xa5
    db 0xa5
    dw 0x8d7
    times 5 db 0xa5
    db 0xa5,3
    times 30 db 0xa5
times 1024-($-$$) db 0
