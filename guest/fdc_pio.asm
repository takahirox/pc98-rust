; Original raw-port non-DMA READ DATA boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/fdc_pio.asm -o fdc_pio.bin
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
mov word [es:0x8001], 0
mov byte [es:0x8003], 0
mov word [es:0x8004], 0
mov byte [es:0x8008], 0
mov word [es:0x13*4], handler
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
mov si, read_data
mov cx, 9
call command
mov di, 0x9000
xor bx, bx
sti
nop
.wait:
cmp byte [es:0x8001], 0
je .wait
cli
cmp sp, 0x7c00
jne fail
; Every result and guard byte must match the independent contract table.
mov si, expected
mov di, 0x8000
mov cx, 64
.check_result:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check_result
xor si, si
.check_payload:
mov ax, si
xor al, ah
cmp al, [es:si+0x9000]
jne fail
inc si
cmp si, 2048
jb .check_payload
mov di, 0x8040
mov cx, 0x0fc0
call guards
mov di, 0x9800
mov cx, 16
call guards
mov si, pass_text
jmp report
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
; Tight byte handler: arrival cadence 16 scheduler slots. No host assistance.
; The two ISR snapshots aggregate actual cascaded CPU acknowledgements.
handler:
in al, 8
or [es:0x8004], al
in al, 0
or [es:0x8005], al
in al, 0x90
test al, 0x20
jz .result
in al, 0x92
stosb
inc bx
mov al, 0x20
out 8, al
out 0, al
iret
.result:
mov [es:0x8018], al              ; EXM clear; seven results available
mov [es:0x8002], bx              ; count of data IRQs
mov [es:0x8006], di              ; exact end of payload
inc byte [es:0x8008]             ; count of separate result IRQs
mov di, 0x8010
in al, 0x92                     ; first result acknowledges FDC request
stosb
in al, 0x90
mov [es:0x8019], al
mov cx, 6
.more:
in al, 0x92
stosb
loop .more
in al, 0x90
mov [es:0x801a], al
mov al, 0x20
out 8, al
out 0, al
in al, 8
mov [es:0x801b], al
in al, 0
mov [es:0x801c], al
mov byte [es:0x8001], 1
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
read_data: db 0x46,0,0,0,2,3,3,0x1b,0xff
pass_text: db 'FDC PIO PASS',0
fail_text: db 'FDC PIO FAIL',0
expected:
db 0xa5,1,0,8,8,0x80,0,0x98,1
 times 7 db 0xa5
 db 0x40,0x80,0,0,0,3,3,0xa5,0xd0,0xd0,0x80,0,0
 times 35 db 0xa5
times 1024-($-$$) db 0
