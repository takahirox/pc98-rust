; Wholly original i486 PC-98 VM86 monitor and guest. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; No DOS, external emulator core, ROM dump, private disk, or host result service.
cpu 486
bits 16
org 0
%define BOOT 0x1fc00
%define RESULT 0x8000
%define TSS 0x9000
%define IDT 0xc000
cli
xor ax, ax
mov ss, ax
mov sp, 0x7c00
push cs
pop es
mov bp, 1024
mov bx, 7168
mov cx, 0x0300
mov dx, 2
mov ax, 0x5690
int 0x1b
jc $
jmp start
times 1024-($-$$) db 0
start:
cli
cld
xor ax, ax
mov ds, ax
mov es, ax
out 0xf2, al
lgdt [cs:gdt_ptr]
lidt [cs:idt_ptr]
mov eax, cr0
or al, 1
mov cr0, eax
jmp dword 8:BOOT+pm32
bits 32
pm32:
mov ax, 16
mov ss, ax
mov ds, ax
mov es, ax
mov esp, 0x7c00
mov edi, RESULT
mov ecx, 16
mov eax, 0xa5a5a5a5
rep stosd
mov dword [RESULT], 0
mov word [RESULT+4], 0
; Stack and data guards, never overwritten by valid frames.
mov [0x17f80], eax
mov [0x18000], eax
mov [0x66ff0], eax
mov [0x67000], eax
mov [0x116ff0], eax
mov [0x117000], eax
; Valid busy guest 32-bit TSS with I/O bitmap and all-ones sentinel.
mov edi, TSS
mov ecx, (0x2081+3)/4
mov eax, -1
rep stosd
mov dword [TSS+4], 0x18000
mov word [TSS+8], 16
mov word [TSS+102], 128
and byte [TSS+128+0x5c/8], ~(1<<(0x5c & 7)) & 255
mov ax, 24
ltr ax
; Monitor and all guest pages initially identity, PDE/PTE U=1.
mov dword [0x10000], 0x11007
mov edi, 0x11000
mov eax, 7
mov ecx, 1024
.pt: stosd
add eax, 4096
loop .pt
; Original guest code and a cross-page prefixed opcode/immediate/return.
mov esi, BOOT+v86
mov edi, 0x40000
mov ecx, v86_end-v86
rep movsb
mov word [0x40ffe], 0xb866
mov dword [0x41000], 0x1234abcd
mov byte [0x41004], 0xc3
; Nonidentity aliases contain exactly the same guest bytes.
mov esi, 0x40000
mov edi, 0x100000
mov ecx, 2048
rep movsd
mov dword [0x53000], 0xcafebabe
mov dword [0x113000], 0xcafebabe
%macro gate 3
mov word [IDT+8*%1], (BOOT+%2-$$) & 0xffff
mov word [IDT+8*%1+2], 8
mov word [IDT+8*%1+4], %3<<8
mov word [IDT+8*%1+6], (BOOT+%2-$$)>>16
%endmacro
gate 13, gp, 0x8e
gate 14, pf, 0x8e
gate 8, fail, 0x8e
gate 0x20, irq, 0x8e
gate 0x30, software, 0xef ; DPL3 trap gate preserves IF
gate 0x31, finish, 0xee
phase:
; Real PC-98 PIC/PIT hardware; IRQ0 becomes pending with VM86 IF=0.
mov al, 0xff
out 0x0a, al
mov al, 0x11
out 0, al
mov al, 0x20
out 2, al
mov al, 0x80
out 2, al
mov al, 1
out 2, al
mov al, 0xfe
out 2, al
mov al, 0x30
out 0x77, al
mov al, 0xe8
out 0x71, al
mov al, 3
out 0x71, al
mov ecx, 2000
.delay: loop .delay
; Every raw VM86 segment differs. ESP high word is deliberately nonzero.
push dword 0x9000 ; GS
push dword 0x0800 ; FS -> result area
push dword 0x5000 ; DS
push dword 0xa000 ; ES -> actual PC-98 text VRAM
push dword 0x6000 ; SS
push dword 0xabcd7000
push dword 0x20002 ; VM=1, IOPL=0, IF=0, PE remains set
push dword 0x4000
push dword 0
entry_iretd: iretd

; Handler EBP addresses the saved EIP slot. PUSHAD protects all VM GPRs.
%macro prolog 1
pushad
lea ebp, [esp+32+%1]
mov ax, 16
mov ds, ax
call check_context
%endmacro
check_context:
cmp dword [ebp+4], 0x4000
jne fail
test dword [ebp+8], 0x20000
jz fail
cmp dword [ebp+12], 0xabcd7000
jne fail
cmp dword [ebp+16], 0x6000
jne fail
cmp dword [ebp+20], 0xa000
jne fail
cmp dword [ebp+24], 0x5000
jne fail
cmp dword [ebp+28], 0x0800
jne fail
cmp dword [ebp+32], 0x9000
jne fail
mov eax, cr0
test al, 1
jz fail
mov [RESULT+8], eax
mov ax, cs
cmp ax, 8
jne fail
pushfd
pop eax
test eax, 0x20100 ; VM and TF clear in protected handler
jnz fail
ret
gp:
prolog 4
cmp dword [ebp-4], 0
jne fail
test dword [ebp+8], 0x10000
jz fail
cmp dword [ebp], vm_cli-v86
je .cli
cmp dword [ebp], vm_out-v86
jne fail
jmp .resume
.cli:
or dword [ebp+8], 0x3000 ; guest monitor grants flags privilege, not I/O privilege
.resume:
inc dword [ebp]
inc byte [RESULT+1]
popad
add esp, 4
iretd
pf:
prolog 4
mov eax, cr2
mov [RESULT+12], eax
cmp eax, 0x53000
je .absent
cmp eax, 0x54000
je .supervisor
cmp eax, 0x55000
jne fail
cmp dword [ebp-4], 7 ; present + user + write
jne fail
or dword [0x11000+0x55*4], 2
jmp .resume
.supervisor:
cmp dword [ebp-4], 5 ; present + user read
jne fail
or dword [0x11000+0x54*4], 4
jmp .resume
.absent:
cmp dword [ebp-4], 4 ; nonpresent + user read
jne fail
mov dword [0x11000+0x53*4], 0x113007
.resume:
invlpg [eax]
inc byte [RESULT+2]
popad
add esp, 4
iretd
irq:
prolog 0
pushfd
pop eax
test eax, 0x200
jnz fail ; interrupt gate cleared IF
test dword [ebp+8], 0x200
jz fail ; saved IF was set
mov al, 0x0b
out 0, al
in al, 0
test al, 1
jz fail
mov al, 0x20
out 0, al
inc byte [RESULT+3]
popad
iretd
software:
prolog 0
pushfd
pop eax
test eax, 0x200
jz fail ; trap gate preserves IF
inc byte [RESULT+4]
popad
iretd
finish:
prolog 0
mov eax, [ebp+8]
mov [RESULT+20], eax
mov eax, [ebp+12]
mov [RESULT+24], eax
mov esi, ebp
add esi, 4
mov edi, RESULT+28
mov eax, [esi]
mov [edi], eax ; saved VM86 CS
mov esi, ebp
add esi, 16
add edi, 4
mov ecx, 5
.snapshot:
mov eax, [esi]
mov [edi], eax
add esi, 4
add edi, 4
loop .snapshot ; SS, ES, DS, FS, GS
mov eax, [ebp-4] ; PUSHAD saved EAX
mov [RESULT+52], eax
mov eax, [ebp-12] ; PUSHAD saved EDX
mov [RESULT+56], eax
cmp edx, 0
jne fail
cmp word [0xa0000], 0x56
jne fail
inc byte [RESULT]
cmp byte [RESULT+5], 0
jne pass
mov byte [RESULT+5], 1
; Switch to PG=1 with nonidentity guest instruction/data/stack pages.
mov dword [0x11000+0x40*4], 0x100007
mov dword [0x11000+0x41*4], 0x101007
mov dword [0x11000+0x50*4], 0x110007
mov dword [0x11000+0x51*4], 0x111007
mov dword [0x11000+0x53*4], 0 ; recoverable nonpresent user #PF
mov dword [0x11000+0x54*4], 0x114003 ; supervisor data, user #PF
mov dword [0x11000+0x55*4], 0x115005 ; user read-only, write #PF
mov dword [0x11000+0x66*4], 0x116007
mov dword [0x11000+0x67*4], 0x117007
mov eax, 0x10000
mov cr3, eax
mov eax, 0xe0010011 ; PE + PG + WP
mov cr0, eax
mov ax, 16
mov es, ax
mov ss, ax
mov esp, 0x7c00
jmp phase
pass:
cmp dword [RESULT], 0x02030402 ; phases=2, GP=4, PF=3, IRQ=2
jne fail
cmp word [RESULT+4], 0x0102 ; software=2, PG phase=1
jne fail
mov esi, BOOT+pass_text
jmp report
fail:
mov esi, BOOT+fail_text
report:
mov ax, 16
mov ds, ax
mov es, ax
mov edi, 0xa0000
.print:
lodsb
test al, al
jz .done
xor ah, ah
stosw
jmp .print
.done: cli
hlt
jmp $

bits 16
v86:
; Guest self-checks execute as actual CPL3 VM86 instructions.
mov eax, esp
cmp eax, 0xabcd7000
jne vm_fail
mov ax, ds
cmp ax, 0x5000
jne vm_fail
mov ax, es
cmp ax, 0xa000
jne vm_fail
mov ax, fs
cmp ax, 0x0800
jne vm_fail
mov ax, gs
cmp ax, 0x9000
jne vm_fail
vm_cli: cli ; monitor must take #GP, then grant IOPL3 and skip this CLI
mov eax, 0x12345678
db 0x67,0x66,0xa3,0x20,0,0,0 ; MOV [a32 DS:20],EAX
cmp dword [0x20], 0x12345678
jne vm_fail
mov [fs:16], eax
pushfd
pop eax
test eax, 0x20000 ; PUSHFD masks VM
jnz vm_fail
and ax, 0x3000
cmp ax, 0x3000
jne vm_fail
push word 2
popf ; cannot lower IOPL
pushf
pop ax
and ax, 0x3000
cmp ax, 0x3000
jne vm_fail
mov dx, 0xf2
vm_out: out dx, al ; bitmap denies even IOPL3, no A20/device side effect
in al, 0x5c ; bitmap-permitted real peripheral input
mov dword [0x0ffe], 0x11223344
cmp dword [0x0ffe], 0x11223344
jne vm_fail
call v86+0x0ffe ; prefix/opcode/immediate crosses separately mapped code pages
cmp eax, 0x1234abcd
jne vm_fail
mov eax, [0x3000] ; PG=1: user nonpresent #PF(4), guest repair/retry
cmp eax, 0xcafebabe
jne vm_fail
mov eax, [0x4000] ; PG=1: user supervisor #PF(5), guest repair/retry
mov dword [0x5000], 0x87654321 ; PG=1: user write #PF(7), guest repair/retry
mov word [es:0], 0x56 ; actual PC-98 text device mapping
sti
nop
mov cx, 10000
.wait:
mov al, [fs:5]
inc al
cmp [fs:3], al
je .irq_done
loop .wait
jmp vm_fail
.irq_done:
int 0x30 ; protected trap gate, guest monitor, IRETD -> VM86
cli
xor edx, edx
int 0x31
vm_fail:
mov edx, 1
int 0x31
jmp $
v86_end:
align 8
gdt:
dq 0
dw 0xffff, 0
db 0, 0x9a, 0xcf, 0
dw 0xffff, 0
db 0, 0x92, 0xcf, 0
dw 0x2080, TSS
db 0, 0x89, 0, 0
.end:
gdt_ptr: dw gdt.end-gdt-1
 dd BOOT+gdt
idt_ptr: dw 0x31*8+7
 dd IDT
pass_text: db 'VM86 PASS',0
fail_text: db 'VM86 FAIL',0
times 8192-($-$$) db 0
