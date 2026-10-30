; Original runnable VM86 synthetic BIOS regression. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. No external guest or ROM bytes.
cpu 486
bits 16
org 0
%define BOOT 0x1fc00
%define RESULT 0x8000
%define PT 0x11000
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
xor eax, eax
rep stosd
mov edi, TSS
mov ecx, (0x2081+3)/4
rep stosd
mov dword [TSS+4], 0x18000
mov word [TSS+8], 16
mov word [TSS+102], 128
mov byte [TSS+0x2080], 255
mov ax, 24
ltr ax
mov dword [0x10000], PT | 7
mov edi, PT
mov eax, 7
mov ecx, 1024
.pt: stosd
add eax, 4096
loop .pt
mov esi, BOOT+v86
mov edi, 0x40000
mov ecx, v86_end-v86
rep movsb
mov esi, 0x40000
mov edi, 0x100000
mov ecx, 1024
rep movsd
%macro gate 3
mov word [IDT+8*%1], (BOOT+%2-$$) & 0xffff
mov word [IDT+8*%1+2], 8
mov word [IDT+8*%1+4], %3<<8
mov word [IDT+8*%1+6], (BOOT+%2-$$)>>16
%endmacro
gate 8, timer, 0x8e
gate 9, keyboard, 0x8e
gate 13, fail, 0x8e
gate 14, pf, 0x8e
gate 0x18, keys, 0xee
gate 0x1b, disk, 0xee
gate 0x1c, clock, 0xee
gate 0x30, finish, 0xee
; Start paged so the unfixed core reproduces the paged VM86 timer stop.
mov byte [RESULT], 1
mov eax, 0x10000
mov cr3, eax
mov eax, 0xe0010011
mov cr0, eax
phase:
mov byte [RESULT+1], 0 ; timer injected fault latch
mov al, 0xfc ; IRQ0/IRQ1, usual PC-98 vectors 08/09
out 2, al
mov al, 0xff
out 0x0a, al
; An actual PIT one-shot enters INT08 before any VM86 software BIOS call.
mov word [0x58a], 0
mov al, 0x30
out 0x77, al
mov al, 0xa0
out 0x71, al
mov al, 0x0f
out 0x71, al
; PG phase 1: missing second buffer page; phase 2: supervisor second page.
cmp byte [RESULT], 0
je .enter
mov dword [PT+0x51*4], 0
cmp byte [RESULT], 2
jne .flush
mov dword [PT+0x51*4], 0x150003
.flush:
mov eax, 0x10000
mov cr3, eax
.enter:
push dword 0x7000
push dword 0
push dword 0x5000
push dword 0x5000
push dword 0x3000
push dword 0xabcd7000
push dword 0x23002 ; VM, IOPL3, IF clear
push dword 0x4000
push dword 0
iretd

; A small original monitor reflects ordinary protected IRQ/INT gates into a
; 16-bit VM86 BIOS frame. It never implements or calls a host BIOS itself.
timer: push dword 8
jmp reflect
keyboard: push dword 9
jmp reflect
keys: push dword 0x18
jmp reflect
disk: push dword 0x1b
jmp reflect
clock: push dword 0x1c
reflect:
pushad
lea ebp, [esp+36]
mov ax, 16
mov ds, ax
mov eax, [ebp-4]
cmp eax, 8
jne .frame
cmp byte [RESULT], 0
je .frame
cmp byte [RESULT+1], 0
jne .frame
mov byte [RESULT+1], 1
mov dword [PT], 5 ; phase 1 user read-only work area
cmp byte [RESULT], 2
jne .invalidate
mov dword [PT], 0 ; phase 2 nonpresent work area
.invalidate:
invlpg [0]
.frame:
mov edi, [ebp+16] ; raw VM86 SS
shl edi, 4
mov edx, [ebp+12] ; preserve ESP high word, wrap only SP
sub dx, 6
movzx ecx, dx
add edi, ecx
mov ecx, [ebp]
mov [edi], cx
mov ecx, [ebp+4]
mov [edi+2], cx
mov ecx, [ebp+8]
mov [edi+4], cx
mov [ebp+12], edx
shl eax, 2
add eax, 0x1000
mov [ebp], eax
mov dword [ebp+4], 0xf000
and dword [ebp+8], ~0x10300 ; clear RF/TF/IF, retain VM/IOPL
popad
add esp, 4
iretd

pf:
pushad
lea ebp, [esp+36] ; EIP above error code
mov ax, 16
mov ds, ax
cmp dword [ebp+4], 0xf000
jne fail
mov eax, cr2
cmp eax, 0x58a
je .timer
cmp eax, 0x51000
jne fail
cmp dword [ebp], 0x1070 ; INT1C OUT, retried with no partial buffer
jne fail
mov edx, 6
cmp byte [RESULT], 1
je .buffer_code
mov edx, 7
.buffer_code:
cmp [ebp-4], edx
jne fail
; Preflight must leave first page intact even though its two bytes are allowed.
cmp word [0x50ffe], 0xa5a5
jne fail
mov dword [PT+0x51*4], 0x51007
cmp byte [RESULT], 2
jne .resume
mov dword [PT+0x51*4], 0x150007
jmp .resume
.timer:
cmp dword [ebp], 0x1020
jne fail
mov edx, 7
cmp byte [RESULT], 1
je .timer_code
mov edx, 6
.timer_code:
cmp [ebp-4], edx
jne fail
; BIOS must not have acknowledged IRQ0 before its inaccessible tick write.
mov al, 0x0b
out 0, al
in al, 0
test al, 1
jz fail
mov eax, cr2
mov dword [PT], 7
cmp byte [RESULT], 2
jne .resume
mov dword [PT], 0x120007
.resume:
invlpg [eax]
inc byte [RESULT+2]
popad
add esp, 4
iretd

finish:
mov ax, 16
mov ds, ax
mov es, ax
mov ss, ax
mov esp, 0x7c00
inc byte [RESULT+4]
cmp byte [RESULT+4], 3
je .complete
inc byte [RESULT]
cmp byte [RESULT], 2
je .nonidentity
; Final control repeats the BIOS sequence in VM86 with PG=0.
mov eax, 0x60010011
mov cr0, eax
mov byte [RESULT], 0
jmp phase
.complete:
mov byte [RESULT], 3
jmp pass
.nonidentity:
; Retain BIOS work area contents in a separate physical page. Guest code,
; stack and buffer pages are independently translated and noncontiguous.
mov esi, 0
mov edi, 0x120000
mov ecx, 1024
rep movsd
mov dword [PT], 0x120007
mov dword [PT+0x40*4], 0x100007
mov dword [PT+0x36*4], 0x116007
mov dword [PT+0x50*4], 0x130007
mov eax, 0x10000
mov cr3, eax
jmp phase
pass:
cmp byte [RESULT+2], 4
jne fail
mov esi, BOOT+pass_text
jmp report
fail:
mov ax, 16
mov ds, ax
mov esi, BOOT+fail_text
report:
mov ax, 16
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
cld
sti
.first_tick:
cmp word [fs:0x58a], 0
je .first_tick
cli
mov byte [fs:0x8100], 0
; One shot callback executes as VM86 and returns using the IRQ's BIOS frame.
push es
push cs
pop es
mov bx, callback-v86
mov cx, 1
mov ah, 2
int 0x1c
pop es
sti
.wait:
cmp byte [fs:0x8100], 1
jne .wait
cli
; Disk status changes only CF in the saved flags, with a nonzero ESP high word.
std
stc
mov ax, 0x0493 ; invalid unit -> CF set
int 0x1b
jnc vm_fail
pushf
pop ax
test ax, 0x400
jz vm_fail
mov ax, 0x0090 ; reset present floppy -> CF clear
int 0x1b
jc vm_fail
pushf
pop ax
test ax, 0x400
jz vm_fail
cld
; RTC buffer crosses pages; faulting last page must leave the first unchanged.
mov word [es:0xffe], 0xa5a5
mov bx, 0xffe
xor ah, ah
int 0x1c
cmp word [es:0xffe], 0xa5a5
je vm_fail
; BIOS character output buffer also uses the guest mapping.
mov bx, 0x200
mov dx, 0x0010
mov ah, 0x14
int 0x18
cmp byte [es:0x200], 0xa5
je vm_fail
; Nonblocking and blocking keyboard paths plus IRQ1 must remain responsive.
mov ax, 0x0300
int 0x18
mov byte [fs:0x8003], 1 ; host may now inject an original keyboard scan
sti
xor ah, ah
int 0x18
cmp al, 'a'
jne vm_fail
mov ah, 4
mov al, 3 ; key-state group containing the A scan (1Dh)
int 0x18
test ah, 0x20
jz vm_fail
cli
mov byte [fs:0x8003], 0
int 0x30
vm_fail:
mov word [es:0], 0xffff
int 0x31
jmp $
callback:
inc byte [fs:0x8100]
iret
v86_end:

bits 16
align 8
gdt:
dq 0, 0x00cf9b000000ffff, 0x00cf93000000ffff
dw 0x2080, TSS
 db TSS >> 16, 0x89, 0, 0
gdt_ptr: dw $-gdt-1
 dd BOOT+gdt
idt_ptr: dw 0x7ff
 dd IDT
pass_text: db 'VM86 BIOS PASS',0
fail_text: db 'VM86 BIOS FAIL',0
times 8192-($-$$) db 0
