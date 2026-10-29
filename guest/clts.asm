; Original PC-98 CLTS / CR0.TS / #NM lifecycle probe, no OS or numeric FPU.
; SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
cpu 486
bits 16
org 0
%define BOOT 0x1fc00
%define RESULT 0x8000
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov ds, ax
mov es, ax
mov di, RESULT
mov cx, 64
mov al, 0xa5
rep stosb
mov word [0x7b00], 0xa5a5
mov word [0x7c00], 0xa5a5
; First independent pre-fix blocker: no ModR/M or immediate follows CLTS.
initial_clts: db 0x0f,0x06
mov word [7*4], nm_real
mov [7*4+2], cs
mov byte [RESULT+1], 0
mov byte [RESULT+2], 0

; Each fault returns to the instruction's first prefix, and retries it.
; TS is written through BOTH LMSW and MOV CR0. All four supported x87 forms
; must fault before changing metadata, AX or the destination memory.
; Explicit paths make fault addresses independent of assembler size choices.
%macro probe 4
db 0x0f,0x01,0xe0 ; SMSW AX, always word on i486
or ax, 8
db 0x0f,0x01,0xf0 ; LMSW AX
mov %1, %2+%%init
%%init: db 0x66,0x66,0xdb,0xe3
mov eax, cr0
or al, 8
mov cr0, eax
mov %1, %2+%%cw
%%cw: fnstcw [RESULT+%3+4]
mov eax, cr0
or al, 8
mov cr0, eax
mov %1, %2+%%sw
%%sw: fnstsw [RESULT+%3+6]
mov eax, cr0
or al, 8
mov cr0, eax
mov %1, %2+%%ax
%%ax: fnstsw ax
cmp ax, 0
jne %4
mov eax, cr0
or al, 0x0a
mov cr0, eax
mov %1, %2+%%wait
%%wait: db 0x9b ; WAIT traps only for MP && TS
mov eax, cr0
and al, 0xfd
mov cr0, eax
; CLTS must leave EM set: the subsequent FNINIT still faults.
mov eax, cr0
or al, 0x0c
mov cr0, eax
db 0x67,0x0f,0x06
mov eax, cr0
test al, 8
jnz %4
test al, 4
jz %4
mov %1, %2+%%em
%%em: fninit
mov eax, cr0
db 0x0f,0x01,0xe3 ; SMSW BX, no assembler-dependent 66
cmp ax, bx
jne %4
cmp eax, 0x60000010+(%2 != 0)
jne %4
%endmacro
probe di, 0, 0, fail
lgdt [cs:gdt_ptr]
lidt [cs:idt_ptr]
mov eax, cr0
or al, 1
mov cr0, eax
jmp dword 8:BOOT+pm32
nm_real:
push eax
push bp
mov bp, sp
cmp [ss:bp+6], di
jne fail
mov ax, cs
cmp [ss:bp+8], ax
jne fail
inc byte [RESULT+1]
db 0x0f,0x06
mov eax, cr0
test al, 8
jnz fail
test al, 4
jz .done
inc byte [RESULT+2]
and al, 0xfb
mov cr0, eax
.done:
pop bp
pop eax
iret
fail:
mov ax, 0xa000
mov es, ax
push cs
pop ds
mov si, fail_text
xor di, di
.print:
lodsb
test al, al
jz .done
xor ah, ah
stosw
jmp .print
.done:
cli
hlt
jmp $
bits 32
pm32:
mov ax, 0x10
mov ds, ax
mov es, ax
mov ss, ax
mov esp, 0x7c00
mov byte [RESULT+17], 0
mov byte [RESULT+18], 0
probe edi, BOOT, 16, fail32
mov esi, BOOT+expected
mov edi, RESULT
mov ecx, 64
repe cmpsb
jne fail32
mov esi, BOOT+pass_text
jmp report
fail32:
mov esi, BOOT+fail_text
report:
mov edi, 0xa0000
.print:
lodsb
test al, al
jz .done
mov ah, 0
stosw
jmp .print
.done:
cli
hlt
jmp $
nm_protected:
push eax
cmp [esp+4], edi
jne fail32
cmp dword [esp+8], 8
jne fail32
test dword [esp+12], 0x10000 ; RF in restartable #NM fault frame
jz fail32
inc byte [RESULT+17]
db 0x0f,0x06
mov eax, cr0
test al, 8
jnz fail32
test al, 4
jz .done
inc byte [RESULT+18]
and al, 0xfb
mov cr0, eax
.done:
pop eax
iretd
align 8
gdt:
dq 0
dw 0xffff, 0
db 0, 0x9a, 0xcf, 0 ; 08: flat 32-bit code
dw 0xffff, 0
db 0, 0x92, 0xcf, 0 ; 10: flat 32-bit data
.end:
gdt_ptr: dw gdt.end-gdt-1
 dd BOOT+gdt
idt:
times 7 dq 0
dw (BOOT+nm_protected-$$) & 0xffff, 8
db 0, 0x8e
dw (BOOT+nm_protected-$$) >> 16
.end:
idt_ptr: dw idt.end-idt-1
 dd BOOT+idt
expected:
db 0xa5,6,1,0xa5,0x7f,3,0,0
 times 8 db 0xa5
 db 0xa5,6,1,0xa5,0x7f,3,0,0
 times 40 db 0xa5
pass_text: db 'CLTS PASS',0
fail_text: db 'CLTS FAIL',0
times 1024-($-$$) db 0
