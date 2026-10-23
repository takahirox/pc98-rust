; Original YM2608 timer IRQ probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/opna_irq.asm -o opna_irq.bin
; One 1024-byte synthetic IPL sector at 1FC0:0000. No DOS/ROM/samples.
; All inputs are register/port writes and guest elapsed clocks. Host only steps.
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
mov cx, 128
mov al, 0xa5
rep stosb
mov byte [es:0x8001], 0
mov word [es:0x1c*4], handler
mov ax, cs
mov [es:0x1c*4+2], ax
; Reinitialize PICs, normal EOI. Route slave IR4 to vector 1Ch explicitly.
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x18
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
mov al, 0xef
out 10, al
mov ax, 0xff24
call fm_write
mov ax, 0x0325
call fm_write
mov ax, 0xff26
call fm_write
mov di, 0x8010
mov bp, 0x8040                  ; poll acknowledgement records
mov bl, 1                       ; Timer A then Timer B
.timer:
mov ax, 0x0029                  ; suppress output before programming timer
call fm_write
call start
call wait_flag
mov ax, 0x0027                  ; stop counter, retain flag
call fm_write
call status
stosb
call status                     ; nondestructive repeated read
stosb
call irr
stosb                            ; clean PIC: no request
mov ah, bl
mov al, 0x29
call fm_write                    ; expose already pending flag
call irr
stosb                            ; IR4 latched on ordinary ticks
mov ax, 0x8029                  ; SCH enabled, timers suppressed
call fm_write
call irr
stosb                            ; disabling device preserves PIC request
call deliver                    ; actual 1Ch handler, records status/ISRs
add di, 3
mov al, [es:0x8001]
stosb
mov al, 0x0b
out 8, al
in al, 8
stosb                            ; ISR clear after guest EOIs
call irr
stosb                            ; no request after suppression + EOI
; Recovery: new overflow, then deliver with the device output still enabled.
mov ax, 0x0029
call fm_write
call start
call wait_flag
mov ax, 0x0027
call fm_write
mov ah, bl
mov al, 0x29
call fm_write
call deliver
add di, 3
mov al, [es:0x8001]
stosb
call irr
stosb
shl bl, 1
cmp bl, 4
jb .timer
cmp sp, 0x7c00
jne fail
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

start:
mov ah, bl
mov al, ah
shl ah, 1
shl ah, 1
or ah, al
mov al, 0x27
call fm_write
ret
wait_flag:
mov cx, 4096                    ; bounded poll, ample for B's 2308 CPU clocks
.poll:
call status
cmp al, bl
je .done
loop .poll
jmp fail
.done:
ret
status:
mov dx, 0x188
in al, dx
and al, 3
ret
irr:
mov al, 0x0a
out 8, al
in al, 8
and al, 0x10
ret
deliver:
mov bh, [es:0x8001]
mov cx, 4096
sti
.wait:
cmp bh, [es:0x8001]
jne .done
loop .wait
cli
jmp fail
.done:
cli
ret
handler:
push ax
push dx
push di
call status
stosb
mov al, 0x0b
out 8, al
in al, 8
stosb
mov al, 0x0b
out 0, al
in al, 0
stosb
mov ax, 0x0029                  ; suppress output, preserving PIC state
call fm_write
mov ax, 0x3027                  ; independent device flag acknowledgement
call fm_write
inc byte [es:0x8001]
mov al, 0x20
out 8, al
out 0, al                       ; both EOIs required for subsequent delivery
; The existing scheduler samples an enabled source again after CPU INTA.
; Drain that retained request through ordinary PIC poll acknowledgement/EOI.
; First path (suppressed before INTA): 00,00; enabled recovery: 87,84.
mov al, 0x0c
out 0, al
in al, 0
mov [es:bp], al
inc bp
mov al, 0x0c
out 8, al
in al, 8
mov [es:bp], al
inc bp
mov al, 0x20
out 8, al
out 0, al
pop di
pop dx
pop ax
iret
fm_write:                       ; AL=register, AH=data
push ax
mov dx, 0x188
out dx, al
add dx, 2
mov al, ah
out dx, al
pop ax
ret
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
jz .halt
xor ah, ah
stosw
jmp .print
.halt:
hlt
jmp .halt
pass_text: db 'OPNA IRQ PASS',0
fail_text: db 'OPNA IRQ FAIL',0
expected:
db 0xa5,4
times 14 db 0xa5
db 1,1,0,0x10,0x10,1,0x10,0x80,1,0,0,1,0x10,0x80,2,0
db 2,2,0,0x10,0x10,2,0x10,0x80,3,0,0,2,0x10,0x80,4,0
times 16 db 0xa5
db 0,0,0x87,0x84,0,0,0x87,0x84
times 56 db 0xa5
times 1024-($-$$) db 0
