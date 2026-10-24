; Original V30 ADD4S boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/v30_add4s.asm -o v30_add4s.bin
; IPL at 1FC0:0000. Guest owns 8000h..801fh and operands at 9001h/9011h.
bits 16
cpu 186
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov ds, ax
mov es, ax
mov al, 0xff
out 2, al
out 0x0a, al
mov di, 0x8000
mov cx, 32
mov al, 0xa5
rep stosb
mov byte [0x9000], 0xa5
mov byte [0x9003], 0x5a
mov byte [0x9010], 0xa5
mov byte [0x9013], 0x5a
mov bp, cases
next_case:
mov ax, [cs:bp]
mov [0x9001], ax
mov ax, [cs:bp+2]
mov [0x9011], ax
mov cx, [cs:bp+4]
mov si, 0x9001
mov di, 0x9011
mov ax, 0x1234
mov bx, 0x4567
mov dx, 0x6789
push word [cs:bp+6]
popf
; Two bytes, no ModR/M. Incoming CF is deliberately set in several cases.
db 0x0f, 0x20
nop
pushf
pop word [cs:observed]
cmp ax, 0x1234
jne fail
cmp bx, 0x4567
jne fail
cmp dx, 0x6789
jne fail
cmp cx, [cs:bp+4]
jne fail
cmp si, 0x9001
jne fail
cmp di, 0x9011
jne fail
mov ax, ds
or ax, ax
jnz fail
mov ax, es
or ax, ax
jnz fail
mov ax, [0x9001]
cmp ax, [cs:bp]
jne fail
mov ax, [0x9011]
cmp ax, [cs:bp+8]
jne fail
mov ax, [cs:observed]
; Only defined CF/ZF and preserved IF/DF. AF/OF/PF/SF are undefined.
and ax, 0x0641
cmp ax, [cs:bp+10]
jne fail
cmp byte [0x9000], 0xa5
jne fail
cmp byte [0x9003], 0x5a
jne fail
cmp byte [0x9010], 0xa5
jne fail
cmp byte [0x9013], 0x5a
jne fail
cmp sp, 0x7c00
jne fail
mov di, [cs:result_pointer]
mov ax, [0x9011]
mov [di], ax
mov ax, [cs:observed]
and ax, 0x0641
mov [di+2], ax
mov ax, [0x9001]
mov [di+4], ax
add word [cs:result_pointer], 6
add bp, 12
cmp bp, cases_end
jb next_case
cld
push cs
pop ds
mov si, expected
mov di, 0x8000
mov cx, 32
check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop check
mov si, pass_text
call report
hlt
jmp $
fail:
cli
cld
push cs
pop ds
mov si, fail_text
call report
hlt
jmp $
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
observed: dw 0
result_pointer: dw 0x8001
; source, destination, CX (varied CH), initial flags, result, masked flags.
; Literal distinguishing examples, odd high digit, last-byte-zero/nonzero result.
cases:
dw 0x0009, 0x0001, 0xab02, 0xf0d7, 0x0010, 0x0000
dw 0x0001, 0x9999, 0x5a04, 0xf402, 0x0000, 0x0441
dw 0x0001, 0x0999, 0xcd03, 0xf4d7, 0x1000, 0x0400
dw 0x8001, 0x1999, 0x1203, 0xf003, 0x0000, 0x0041
dw 0x0000, 0x0001, 0xef04, 0xf403, 0x0001, 0x0400
cases_end:
expected:
db 0xa5
dw 0x0010, 0x0000, 0x0009
dw 0x0000, 0x0441, 0x0001
dw 0x1000, 0x0400, 0x0001
dw 0x0000, 0x0041, 0x8001
dw 0x0001, 0x0400, 0x0000
db 0xa5
pass_text: db 'V30 ADD4S PASS', 0
fail_text: db 'V30 ADD4S FAIL', 0
times 1024-($-$$) db 0
