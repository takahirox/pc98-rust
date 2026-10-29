; Original PC-98 TR/TSS initialization guest. No task switch or guest OS.
; SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
bits 16
cpu 486
org 0
%define BOOT 0x1fc00
%define RESULT 0x8000
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov di, RESULT
mov cx, 64
mov al, 0xa5
rep stosb
lgdt [gdt_ptr]
mov eax, cr0
or al, 1
mov cr0, eax
jmp 8:pm16
pm16:
mov ax, 0x18
mov ds, ax
mov es, ax
; STR/LTR use explicit bytes to avoid assembler-version prefix choices.
; LTR AX, 0F 00 /3, is the initial blocker.
mov eax, 0xface0028
load16: db 0x0f,0x00,0xd8
mov ebx, 0x1234ffff
; 66 must not widen STR on the target i386/i486.
db 0x66,0x0f,0x00,0xcb        ; STR BX
cmp ebx, 0x12340028
jne fail16
mov [RESULT+1], bx
a32 cmp byte [BOOT+gdt+0x2d], 0x83
jne fail16
mov byte [RESULT+3], 0x16
; Guest edits the descriptor after LTR. STR still reads visible TR.
a32 mov byte [BOOT+gdt+0x2d], 0
db 0x0f,0x00,0x0e           ; STR word [RESULT+5]
 dw RESULT+5
cmp word [RESULT+5], 0x28
jne fail16
jmp dword 0x10:BOOT+pm32
bits 32
pm32:
; Source width stays a word, with both 66 and 67 overrides.
mov word [RESULT+9], 0x33    ; RPL is ignored by LTR, retained by STR.
mov bx, RESULT+9
db 0x66,0x67,0x0f,0x00,0x1f ; LTR [BX]
mov eax, 0xbeefffff
db 0x0f,0x00,0xc8           ; STR AX, default operand size 32
cmp eax, 0xbeef0033
jne fail32
mov [RESULT+11], ax
db 0x0f,0x00,0x0d           ; STR word [RESULT+15]
 dd RESULT+15
cmp byte [BOOT+gdt+0x35], 0xeb ; DPL=3 is ignored, busy=1.
jne fail32
mov byte [RESULT+17], 0x32
; Reload is guest-owned: explicitly clear busy before another LTR.
and byte [BOOT+gdt+0x35], 0xfd
mov word [RESULT+19], 0x30
db 0x0f,0x00,0x1d           ; LTR word [RESULT+19]
 dd RESULT+19
mov ecx, 0xcafef00d
db 0x66,0x0f,0x00,0xc9      ; STR CX
cmp ecx, 0xcafe0030
jne fail32
mov [RESULT+21], cx
cmp byte [BOOT+gdt+0x35], 0xeb
jne fail32
; Preserve flags across both instructions.
push dword 0x893
popfd
pushfd
pop edx
ltr_again:
and byte [BOOT+gdt+0x35], 0xfd ; changes flags, so restore them below
push edx
popfd
db 0x0f,0x00,0x1d           ; LTR word [RESULT+19]
 dd RESULT+19
db 0x0f,0x00,0x0d           ; STR word [RESULT+25]
 dd RESULT+25
pushfd
pop eax
cmp eax, edx
jne fail32
mov byte [RESULT+27], 0x46
jmp word 8:pm16_exit
fail32:
jmp word 8:fail16
bits 16
pm16_exit:
mov eax, cr0
and al, 0xfe
mov cr0, eax
jmp 0x1fc0:real_again
fail16:
mov ax, 0xa000
mov es, ax
mov word [es:0], 'F'
cli
hlt
jmp $
real_again:
push cs
pop ds
xor ax, ax
mov es, ax
mov si, expected
mov di, RESULT
mov cx, 64
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
; Reenter PE without reset: cached TR survives PE clear/set.
mov eax, cr0
or al, 1
mov cr0, eax
jmp 8:reenter
reenter:
db 0x0f,0x00,0xc8           ; STR AX
cmp ax, 0x30
jne fail16
mov eax, cr0
and al, 0xfe
mov cr0, eax
jmp 0x1fc0:finish
finish:
mov si, pass_text
jmp report
fail:
mov si, fail_text
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
cli
hlt
jmp $
align 8
gdt:
dq 0
dw 0xffff, BOOT & 0xffff
db BOOT >> 16, 0x9a, 0, 0  ; 08: 16-bit code
dw 0xffff, 0
db 0, 0x9a, 0xcf, 0        ; 10: flat 32-bit code
dw 0xffff, 0
db 0, 0x92, 3, 0          ; 18: 16-bit data, limit 3ffffh
; unused slot
dq 0
dw 43, 0x8100
db 0, 0x81, 0, 0          ; 28: available 16-bit TSS, 44 bytes
dw 103, 0x8200
db 0, 0xe9, 0, 0          ; 30: available 32-bit TSS, 104 bytes, DPL3
.end:
gdt_ptr: dw gdt.end-gdt-1
 dd BOOT+gdt
expected:
db 0xa5,0x28,0,0x16,0xa5,0x28,0,0xa5,0xa5,0x33,0,0x33,0
 db 0xa5,0xa5,0x33,0,0x32,0xa5,0x30,0,0x30,0
 db 0xa5,0xa5,0x30,0,0x46
 times 36 db 0xa5
pass_text: db 'TASK REGISTER PASS',0
fail_text: db 'TASK REGISTER FAIL',0
times 1024-($-$$) db 0
