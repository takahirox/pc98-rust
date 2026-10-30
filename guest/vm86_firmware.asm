; Original MIT paged VM86 synthetic BIOS probe. No DOS or private-media bytes.
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
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
mov dword [RESULT], 0
mov dword [RESULT+4], 0
mov edi, TSS
mov ecx, (0x2081+3)/4
xor eax, eax
rep stosd
mov dword [TSS+4], 0x18000
mov word [TSS+8], 16
mov word [TSS+102], 128
mov byte [TSS+0x2080], 255
mov ax, 24
ltr ax
mov dword [0x10000], 0x11007
mov edi, 0x11000
mov eax, 7
mov ecx, 1024
.pt: stosd
add eax, 4096
loop .pt
mov esi, BOOT+v86
mov edi, 0x40000
mov ecx, v86_end-v86
rep movsb
; Nonidentity low BIOS work area, VM stack and split RTC destination.
mov dword [0x11000+0x300*4], 3 ; supervisor physical-zero alias
mov dword [0x11000], 0x100005 ; user read-only: recover timer write #PF(7)
mov dword [0x11000+0x36*4], 0x11f007
mov dword [0x11000+0x50*4], 0x130007
mov dword [0x11000+0x200*4], 0x150007
mov dword [0x11000+0x210*4], 0x160007
mov dword [0x150000], 0x6c8e13a7
mov dword [0x160000], 0
mov dword [0x11000+0x51*4], 0x140000 ; absent second destination page
mov word [0x10058a], 41
mov word [0x58a], 0xbeef
mov byte [0x130ffc], 0xa5
mov byte [0x140003], 0xa5
mov dword [0x37000], 0xa5a5a5a5
mov dword [0x120040], 0xa5a5a5a5
%macro gate 2
mov word [IDT+8*%1], (BOOT+%2-$$) & 0xffff
mov word [IDT+8*%1+2], 8
mov word [IDT+8*%1+4], 0xee00
mov word [IDT+8*%1+6], (BOOT+%2-$$)>>16
%endmacro
gate 12, fail
gate 13, fail
gate 14, pf
gate 0x31, finish
mov eax, 0x10000
mov cr3, eax
mov eax, 0xe0010011
mov cr0, eax
push dword 0
push dword 0x0800 ; FS results (identity)
push dword 0x5000 ; DS
push dword 0x5000 ; ES
push dword 0x3000 ; SS
push dword 0x7000 ; ESP
push dword 0x23002 ; VM, IOPL3, IF=0
push dword 0x4000
push dword 0
iretd
pf:
; Same restartable host service instruction must be retried after monitor repair.
mov ebp, esp
pushad
mov ax, 16
mov ds, ax
mov eax, cr2
cmp byte [RESULT+1], 0
jne .rtc
cmp eax, 0x58a
jne fail
cmp dword [ebp], 7
jne fail
cmp dword [ebp+4], 0x1020
jne fail
cmp dword [ebp+8], 0xf000
jne fail
cmp word [0x10058a], 41
jne fail
or dword [0x11000], 2
jmp .resume
.rtc:
cmp byte [RESULT+1], 1
jne fail
cmp eax, 0x51000
jne fail
cmp dword [ebp], 6
jne fail
cmp dword [ebp+4], 0x1070
jne fail
cmp word [0x130ffd], 0 ; first page unchanged after failed multi-page service
jne fail
mov dword [0x11000+0x51*4], 0x140007
.resume:
invlpg [eax]
inc byte [RESULT+1]
popad
add esp, 4
iretd
finish:
mov ax, 16
mov ds, ax
cmp edx, 0
jne fail
cmp byte [RESULT+5], 1
jne fail
cmp dword [0x160000], 0x6c8e13a7
jne fail
cmp byte [RESULT+1], 2
jne fail
cmp word [0x30058a], 0xbeef
jne fail
cmp word [0x10058a], 1
jne fail
cmp dword [0x37000], 0xa5a5a5a5
jne fail
cmp dword [0x120040], 0xa5a5a5a5
jne fail
cmp byte [0x130ffc], 0xa5
jne fail
cmp byte [0x140003], 0xa5
jne fail
mov byte [RESULT], 2
mov esi, BOOT+pass_text
jmp report
fail:
mov ax, 16
mov ds, ax
mov byte [RESULT], 0xff
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
pushf
call 0xf000:0x1020 ; timer BIOS through real ROM OUT/IRET
mov ax, 0
mov bx, 0xffd
pushf
call 0xf000:0x1070 ; RTC spans two noncontiguous pages, second initially absent
; Invalid HDD query returns carry in the mapped saved FLAGS.
mov ax, 0x0480
clc
pushf
call 0xf000:0x106c
jnc bad
cmp ah, 0x80
jne bad
; Valid 386 data descriptors with 20-bit limits: upper limit nibble is
; neither a reserved bit nor an instruction to reject the BIOS request.
mov dword [0x210], 0x0000ffff
mov dword [0x214], 0x000f9320
mov dword [0x218], 0x0000ffff
mov dword [0x21c], 0x000f9321
mov bx, 0x200
mov cx, 4
xor si, si
xor di, di
mov ax, 0x9000
pushf
call 0xf000:0x107c
jc bad
mov byte [fs:5], 1
; Arm callback and let real bus time advance with IF=0 (no IRQ reflection).
mov ax, 0x0200
mov bx, callback-v86
mov cx, 1
push cs
pop es
pushf
call 0xf000:0x1070
mov cx, 12000
.delay: loop .delay
pushf
call 0xf000:0x1020 ; tail callback reuses return frame, remains CPL3/VM86
cmp byte [fs:RESULT-0x8000+4], 1
jne bad
; Arming resets BIOS ticks; the subsequent timer call increments it once.
mov ax, 0
mov es, ax
cmp word [es:0x58a], 1
jne bad
mov byte [fs:0], 1 ; ready: host injects Enter through real keyboard FIFO
.wait:
pushf
call 0xf000:0x1024 ; consume wire FIFO through keyboard IRQ service
mov ah, 1
pushf
call 0xf000:0x1060
cmp bh, 1
jne .wait
mov ah, 0
pushf
call 0xf000:0x1060
cmp ax, 0x1c0d
jne bad
mov [fs:6], ax
xor edx, edx
int 0x31
bad:
mov edx, 1
int 0x31
jmp $
callback:
inc byte [fs:4]
iret
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
pass_text: db 'VM86 BIOS PASS',0
fail_text: db 'VM86 BIOS FAIL',0
times 8192-($-$$) db 0
