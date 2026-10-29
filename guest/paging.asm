; Original nonidentity Intel486 paging / #PF / invalidation boot probe.
; SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
cpu 486
bits 16
org 0
%define BOOT 0x1fc00
%define VIRTUAL 0x400000
%define RESULT 0x8000
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov ds, ax
mov es, ax
out 0xf2, al ; PC-98 A20 enable
lgdt [cs:gdt_ptr]
lidt [cs:idt_ptr]
mov eax, cr0
or al, 1
mov cr0, eax
jmp dword 8:BOOT+pm32
bits 32
pm32:
mov ax, 16
mov ds, ax
mov es, ax
mov ss, ax
mov esp, 0x7c00
mov edi, RESULT
mov ecx, 16
mov eax, 0xa5a5a5a5
rep stosd
; Guard both ends of the nonidentity stack and adjacent data pages.
mov [0x102000], eax
mov [0x102ffc], eax
mov edi, 0x10000
xor eax, eax
mov ecx, 3072
rep stosd
mov dword [0x10000], 0x11003
mov dword [0x10004], 0x12003
mov edi, 0x11000
mov eax, 3
mov ecx, 1024
.identity:
stosd
add eax, 4096
loop .identity
mov dword [0x12000], 0x100003 ; code: nonidentity physical 1 MiB
mov dword [0x12004], 0x101003 ; data
mov dword [0x12008], 0x102003 ; stack
mov esi, BOOT
mov edi, 0x100000
mov ecx, 256
rep movsd
mov eax, 0x10018 ; CR3 PCD/PWT retained
mov cr3, eax
mov eax, 0x60010011
mov cr0, eax
or eax, 0x80000000
enable_paging: mov cr0, eax
jmp dword 8:VIRTUAL+paged
paged:
mov esp, 0x402ff0
mov eax, cr0
mov [RESULT+4], eax
cmp eax, 0xe0010011
jne fail
mov eax, cr3
mov [RESULT+8], eax
cmp eax, 0x10018
jne fail
smsw ax
cmp ax, 0x11
jne fail
mov dword [0x401000], 0x11223344
cmp dword [0x101000], 0x11223344
jne fail
push dword 0x76543210
pop ebx
cmp ebx, 0x76543210
jne fail
mov byte [RESULT+1], 0
retry: mov dword [0x403000], 0xcafebabe
cmp dword [0x103000], 0xcafebabe
jne fail
mov eax, cr2
mov [RESULT+12], eax
cmp eax, 0x403000
jne fail
cmp byte [RESULT+1], 1
jne fail
; Existing TLB entry must stay stale until explicit invalidation.
mov dword [0x104000], 0x55667788
mov ebx, [0x401000] ; prime after identity aliases used above
mov dword [0x12004], 0x104003
cmp dword [0x401000], 0x11223344
jne fail
invlpg [0x401000]
cmp dword [0x401000], 0x55667788
jne fail
mov dword [0x12004], 0x101003
mov eax, cr3
mov cr3, eax ; reload same value flushes
cmp dword [0x401000], 0x11223344
jne fail
jmp word 24:paged16
bits 16
paged16:
mov eax, [dword 0x401000] ; independent 66/67 with 16-bit protected CS
cmp eax, 0x11223344
jne fail16
push word 0x4321
pop bx
cmp bx, 0x4321
jne fail16
jmp dword 8:VIRTUAL+after16
fail16: jmp dword 8:VIRTUAL+fail
bits 32
after16:
mov [RESULT+16], eax
cmp dword [0x102000], 0xa5a5a5a5
jne fail
cmp dword [0x102ffc], 0xa5a5a5a5
jne fail
cmp dword [0x103000], 0xcafebabe
jne fail
mov eax, [0x103000]
mov [RESULT+20], eax
jmp dword 8:BOOT+disable
disable:
mov eax, cr0
and eax, 0x7fffffff
mov cr0, eax
mov [RESULT+24], eax
cmp eax, 0x60010011
jne fail
mov esp, 0x7c00
mov esi, BOOT+pass_text
jmp report
fail:
mov esi, BOOT+fail_text
report:
mov edi, 0xa0000
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
pf_handler:
cmp dword [esp], 2
jne fail
cmp dword [esp+4], VIRTUAL+retry
jne fail
cmp dword [esp+8], 8
jne fail
test dword [esp+12], 0x10000
jz fail
push eax
mov eax, cr2
cmp eax, 0x403000
jne fail
inc byte [RESULT+1]
mov dword [0x1200c], 0x103003
invlpg [0x403000] ; no operand access to the absent page
pop eax
add esp, 4
iretd
align 8
gdt:
dq 0
dw 0xffff, 0
db 0, 0x9a, 0xcf, 0 ; flat 32-bit code
dw 0xffff, 0
db 0, 0x92, 0xcf, 0 ; flat 32-bit data/stack
dw 0xffff, 0
db 0x40, 0x9a, 0, 0 ; 16-bit code, base 00400000h
.end:
gdt_ptr: dw gdt.end-gdt-1
 dd BOOT+gdt
idt:
times 14 dq 0
dw (BOOT+pf_handler-$$) & 0xffff, 8
db 0, 0x8e
dw (BOOT+pf_handler-$$) >> 16
.end:
idt_ptr: dw idt.end-idt-1
 dd BOOT+idt
pass_text: db 'PAGING PASS',0
fail_text: db 'PAGING FAIL',0
times 1024-($-$$) db 0
