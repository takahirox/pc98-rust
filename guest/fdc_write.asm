; Original raw-port non-DMA WRITE DATA and readback boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/fdc_write.asm -o fdc_write.bin
; Synthetic IPL loads this 1024-byte sector at 1FC0:0000.
bits 16
cpu 8086
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov di, 0x8000
mov cx, 0x1810
mov al, 0xa5
rep stosb
mov bp, 0x8000
call init_record
mov bp, 0x8040
call init_record
mov bp, 0x8000
mov word [es:0x13*4], write_handler
mov ax, cs
mov [es:0x13*4+2], ax
; Normal EOI; fixed master IR7/slave IR3, actual INT 13h delivery.
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x10
out 10, al
mov al, 0x80
out 2, al
mov al, 7
out 10, al
mov al, 1
out 2, al
out 10, al
mov al, 0x7f
out 2, al
mov al, 0xf7
out 10, al
mov al, 0x0b
out 0, al
out 8, al
mov al, 6
out 0x15, al                     ; masked DMA channel 2
mov al, 4
out 0x11, al                     ; AND disabled DMA controller
mov al, 0x18
out 0x94, al
mov si, specify
mov cx, 3
call command
.wait_specify:
in al, 0x90
cmp al, 0x80
jne .wait_specify
mov di, 0x9000
mov cx, 1024
mov al, 71
.fill:
stosb
add al, 29
loop .fill
mov si, write_data
mov cx, 9
call command
mov si, 0x9000
xor bx, bx
sti
nop
.wait_write:
cmp byte [es:0x8001], 0
je .wait_write
cli
; The result request has been drained and both PICs EOI'd by the guest.
mov word [es:0x13*4], read_handler
mov bp, 0x8040
mov si, read_data
mov cx, 9
call command
mov di, 0x9400
xor bx, bx
sti
nop
.wait_read:
cmp byte [es:0x8041], 0
je .wait_read
cli
cmp sp, 0x7c00
jne fail
mov si, expected
mov di, 0x8000
mov cx, 128
.check_result:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check_result
mov di, 0x9000
call payload
mov di, 0x9400
call payload
mov di, 0x8080
mov cx, 0x0f80
call guards
mov di, 0x9800
mov cx, 16
call guards
mov si, pass_text
jmp report
init_record:
mov word [es:bp+1], 0
mov byte [es:bp+3], 0
mov word [es:bp+4], 0
mov byte [es:bp+8], 0
ret
payload:
mov cx, 1024
mov al, 71
.check:
cmp al, [es:di]
jne fail
inc di
add al, 29
loop .check
ret
command:
lodsb
out 0x92, al
loop command
ret
guards:
cmp byte [es:di], 0xa5
jne fail
inc di
loop guards
ret
; Separate direction handlers keep byte service below the 104-clock deadline.
; Both record real PIC ISR bits and require normal EOIs on every request.
%macro byte_handler 2
%1:
in al, 8
or [es:bp+4], al
in al, 0
or [es:bp+5], al
in al, 0x90
test al, 0x20
jz %%result
%if %2
mov al, [es:si]
out 0x92, al
inc si
%else
in al, 0x92
stosb
%endif
inc bx
mov al, 0x20
out 8, al
out 0, al
iret
%%result:
%if %2
mov [es:bp+6], si
%else
mov [es:bp+6], di
%endif
jmp result_handler
%endmacro
byte_handler write_handler, 1
byte_handler read_handler, 0
result_handler:
mov [es:bp+24], al
mov [es:bp+2], bx
inc byte [es:bp+8]
mov di, bp
add di, 16
in al, 0x92
stosb
in al, 0x90
mov [es:bp+25], al
mov cx, 6
.more:
in al, 0x92
stosb
loop .more
in al, 0x90
mov [es:bp+26], al
mov al, 0x20
out 8, al
out 0, al
in al, 8
mov [es:bp+27], al
in al, 0
mov [es:bp+28], al
mov byte [es:bp+1], 1
iret
fail:
cli
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
hlt
jmp .done
specify: db 3,0xdf,3
write_data: db 0x45,0,0,0,2,3,2,0x1b,1
read_data: db 0x46,0,0,0,2,3,2,0x1b,1
pass_text: db 'FDC WRITE PASS',0
fail_text: db 'FDC WRITE FAIL',0
expected:
%macro record 1
 db 0xa5,1,0,4,8,0x80,0,%1,1
 times 7 db 0xa5
 db 0x40,0x80,0,0,0,2,3,0xa5,0xd0,0xd0,0x80,0,0
 times 35 db 0xa5
%endmacro
record 0x94
record 0x98
times 1024-($-$$) db 0
