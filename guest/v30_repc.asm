; Original V30 REPC/REPNC probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/v30_repc.asm -o v30_repc.bin
; One IPL sector at 1FC0:0000. Results/guards at 8000h..805Fh.
bits 16
cpu 186
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov al, 0xff
out 2, al
out 0x0a, al
mov di, 0x8000
mov cx, 96
mov al, 0xa5
rep stosb
mov si, destination
mov di, 0x9000
mov cx, 32
rep movsb
mov bx, 0x8001
; Each record is CX, SI, DI, and all six defined comparison flags.
; Explicit bytes keep NEC prefixes independent of assembler mnemonic support.
%macro probe 7
    mov si, %3
    mov di, %4
    mov cx, %5
    mov ax, 5
    push word %6
    popf
    %if %7
        db 0x2e, %1, %2 ; CS source before repeat prefix
    %else
        db %1, 0x2e, %2 ; CS source after repeat prefix
    %endif
    pushf
    pop dx
    and dx, 0x08d5
    mov [es:bx], cx
    mov [es:bx+2], si
    mov [es:bx+4], di
    mov [es:bx+6], dx
    add bx, 8
%endmacro
; Incoming CF is opposite to the carry-repeat condition in each nonzero case.
probe 0x65, 0xa6, 0x301, 0x9001, 3, 0xf002, 0
probe 0x64, 0xa6, 0x305, 0x9005, 3, 0xf003, 1
probe 0x65, 0xa7, 0x30b, 0x900b, 3, 0xf402, 1
probe 0x64, 0xa7, 0x311, 0x9011, 3, 0xf003, 0
probe 0x65, 0xae, 0x301, 0x9007, 3, 0xf402, 0
probe 0x64, 0xae, 0x305, 0x9005, 4, 0xf003, 1
probe 0x65, 0xaf, 0x30b, 0x9015, 3, 0xf402, 1
probe 0x64, 0xaf, 0x311, 0x9011, 3, 0xf003, 0
probe 0x65, 0xaf, 0xffff, 0xffff, 0, 0xfcd7, 0
probe 0x64, 0xa6, 0x301, 0x9001, 1, 0xf003, 1
cld
mov byte [es:0x8051], 0x5a
mov si, expected
mov di, 0x8000
mov cx, 96
check_results:
lodsb
cmp al, [es:di]
jne fail
inc di
loop check_results
; Independently check every source/destination byte and their embedded guards.
mov si, source_expected
mov di, 0x300
mov cx, 32
check_source:
lodsb
cmp al, [ds:di]
jne fail
inc di
loop check_source
mov si, destination
mov di, 0x9000
mov cx, 32
check_destination:
lodsb
cmp al, [es:di]
jne fail
inc di
loop check_destination
cmp sp, 0x7c00
jne fail
mov si, pass_text
call report
hlt
jmp $
fail:
cli
cld
mov si, fail_text
call report
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
pass_text: db 'V30 REPC PASS', 0
fail_text: db 'V30 REPC FAIL', 0
; Fixed offsets make expected indices auditable without a host assembler parser.
times 0x300-($-$$) db 0
source:
db 0xa5, 1, 5, 0, 0xa5, 5, 5, 5, 0xa5
dw 5, 1, 0
db 0xa5, 0xa5
dw 5, 5, 5
times 32-($-source) db 0xa5
source_expected:
db 0xa5, 1, 5, 0, 0xa5, 5, 5, 5, 0xa5
dw 5, 1, 0
db 0xa5, 0xa5
dw 5, 5, 5
times 32-($-source_expected) db 0xa5
destination:
db 0xa5, 2, 4, 0, 0xa5, 5, 4, 6, 0xa5
dw 4, 2, 0
db 0xa5, 0xa5
dw 5, 4, 6
times 32-($-destination) db 0xa5
expected:
db 0xa5
dw 1, 0x303, 0x9003, 0
dw 0, 0x308, 0x9008, 0x95
dw 1, 0x307, 0x9007, 0
dw 0, 0x317, 0x9017, 0x95
dw 1, 0x301, 0x9005, 0
dw 1, 0x305, 0x9008, 0x95
dw 1, 0x30b, 0x9011, 0
dw 0, 0x311, 0x9017, 0x95
dw 0, 0xffff, 0xffff, 0x8d5
dw 0, 0x302, 0x9002, 0x95
db 0x5a
times 14 db 0xa5
times 1024-($-$$) db 0
