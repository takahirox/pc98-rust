; Original Intel486 control-write probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/mov_cr_write.asm -o mov_cr_write.bin
bits 16
cpu 486
org 0
%ifdef MODE
; Separate synthetic transition entry; never reports successful continuation.
cli
mov eax, MODE
db 0x66,0x67,0x65,0x0f,0x22,0xc0
hlt
%else
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
mov word [es:6*4], fault
mov word [es:13*4], fault
mov ax, cs
mov [es:6*4+2], ax
mov [es:13*4+2], ax
mov byte [es:0x8061], 0

; Each source must survive, all 32 bits must reach CR2, including ESP.
%macro probe 4
mov %1, %3
push word 0x8d7
popf
%if %4 = 1
 db 0x66,0x66,0x67,0x67,0x64,0x65,0xf2,0xf3
%endif
db 0x0f,0x22,0xd0+%2
cmp %1, %3
jne fail
mov eax, cr2
mov [es:0x8001+8*%2], eax
%if %2 = 4
 mov esp, 0x7c00
%endif
%endmacro
; Record FLAGS immediately around a write, before any comparison.
mov eax, 0x60050032
push word 0x8d7
popf
mov cr0, eax
pushf
pop word [es:0x8059]
mov ebx, cr0
mov [es:0x8041], ebx
smsw [es:0x8051]
probe eax, 0, 0x12340000, 0
probe ecx, 1, 0x12340001, 1
probe edx, 2, 0x12340002, 0
probe ebx, 3, 0x12340003, 1
; Set flags before changing ESP to keep the stack valid.
mov esp, 0x12345018
db 0x66,0x0f,0x22,0xdc
mov eax, cr3
mov [es:0x8049], eax
mov cr2, esp
mov eax, cr2
mov [es:0x8021], eax
mov esp, 0x7c00
probe ebp, 5, 0x12340005, 0
probe esi, 6, 0x12340006, 1
probe edi, 7, 0x12340007, 0
mov eax, 0x60000010
mov cr0, eax

; Handler validates first-prefix restart, flags, frame and source preservation,
; repairs each encoding/value and executes IRET to retry the same instruction.
%macro invalid 3
mov word [expected_ip], %1
mov word [repair_ip], %1+%2
mov byte [replacement], %3
mov eax, 0x60000010
push word 2
popf
%endmacro
invalid locked, 1, 0x65
locked: db 0x66,0xf0,0x0f,0x22,0xc0
invalid reserved, 4, 0xc0
reserved: db 0x67,0x65,0x0f,0x22,0xe0
invalid memory, 4, 0xc0
memory: db 0x66,0x26,0x0f,0x22,0x00
invalid bad_pg, 0, 0x66
mov eax, 0xe0000010
bad_pg: db 0x66,0x0f,0x22,0xc0
cmp byte [es:0x8061], 4
jne fail
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
jmp report
fault:
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
mov si, [cs:repair_ip]
mov al, [cs:replacement]
mov [cs:si], al
inc byte [es:0x8061]
pop si
pop ax
pop bp
; Invalid PG without PE is #GP, not an unsupported transition; repair source.
mov eax, 0x60000010
iret
fail:
cld
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
expected_ip: dw 0
repair_ip: dw 0
replacement: db 0
pass_text: db 'MOV CR WRITE PASS',0
fail_text: db 'MOV CR WRITE FAIL',0
expected:
%assign i 0
%rep 8
 db 0xa5
 %if i = 4
 dd 0x12345018
 %else
 dd 0x12340000+i
 %endif
 times 3 db 0xa5
 %assign i i+1
%endrep
 db 0xa5
 dd 0x60050032
 times 3 db 0xa5
 db 0xa5
 dd 0x12345018
 times 3 db 0xa5
 db 0xa5
 dw 0x32
 times 5 db 0xa5
 db 0xa5
 dw 0x8d7
 times 5 db 0xa5
 db 0xa5,4
 times 30 db 0xa5
%endif
times 1024-($-$$) db 0
