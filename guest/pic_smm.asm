; Original OCW3 special-mask hardware-IRQ probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pic_smm.asm -o pic_smm.bin
; Synthetic IPL loads one 1024-byte sector at 1FC0:0000. No software INTs.
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
mov cx, 32
mov al, 0xa5
rep stosb
mov byte [es:0x8001], 0          ; input handshake
mov byte [es:0x8002], 0          ; trace count
mov byte [es:0x8003], 0          ; timer handler count
mov word [es:8*4], timer
mov word [es:9*4], keyboard
mov ax, cs
mov [es:8*4+2], ax
mov [es:9*4+2], ax
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1                      ; AEOI disabled
out 2, al
mov al, 0xfc                   ; only timer and keyboard
out 2, al
mov al, 0xff
out 0x0a, al
mov al, 0xc0                   ; IR0 lowest, keyboard outermost
out 0, al
mov al, 0x30                   ; one-shot PIT channel 0, divisor 1000
out 0x77, al
mov al, 0xe8
out 0x71, al
mov al, 3
out 0x71, al
inc byte [es:0x8001]            ; host sends physical Return down once
.wait_competing:
mov al, 0x0a
out 0, al
in al, 0
and al, 3
cmp al, 3
jne .wait_competing
mov [es:0x8005], al             ; guest confirms timer AND keyboard pending
sti
nop
.wait_done:
cmp byte [es:0x8001], 2
jne .wait_done
cli
mov si, expected
mov di, 0x8000
mov cx, 32
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
mov si, pass_text
call report
hlt
jmp $

keyboard:
; Drain immediately, before another bus tick can re-latch the FIFO request.
in al, 0x41
mov [es:0x8004], al
push ax
push bx
push cx
mov al, 1
call log_irq                    ; trace 01, ISR 02
mov al, 0xfe                    ; mask own ISR1; timer remains unmasked
out 2, al
in al, 2
mov [es:0x8011], al
sti
nop
mov cx, 256                    ; IF=1 observation window in normal mode
.normal_blocked:
cmp byte [es:0x8003], 0
jne fail
loop .normal_blocked
cli
mov al, [es:0x8003]
mov [es:0x8006], al             ; zero timer entries before SMM
mov al, 0x0a
out 0, al
in al, 0
and al, 3
mov [es:0x8010], al
cmp al, 1                      ; timer is still pending, not lost or masked
jne fail
mov al, 0x68
out 0, al
sti
nop
.wait_nested:
cmp byte [es:0x8003], 1
jb .wait_nested
jne fail
cli
mov al, [es:0x8003]
mov [es:0x8007], al             ; exactly one timer entry after SMM
mov al, 1
call log_irq                    ; trace 01, ISR 02 after nested EOI
mov al, 0x48
out 0, al
mov al, 0x61                   ; specific EOI clears masked outer ISR1
out 0, al
mov al, 0x0b
out 0, al
in al, 0
mov [es:0x8013], al
mov al, 0x0a
out 0, al
in al, 0
and al, 3
mov [es:0x8012], al
inc byte [es:0x8001]
pop cx
pop bx
pop ax
iret

timer:
push ax
push bx
inc byte [es:0x8003]
xor al, al
call log_irq                    ; trace 00, ISR 03 while keyboard still active
mov al, 0x20                   ; nonspecific EOI must SKIP masked ISR1
out 0, al
pop bx
pop ax
iret

log_irq:
xor bx, bx
mov bl, [es:0x8002]
cmp bl, 3
jae fail
mov [es:bx+0x8008], al
mov al, 0x0b
out 0, al
in al, 0
mov [es:bx+0x800c], al
inc byte [es:0x8002]
ret

fail:
cli
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
pass_text: db 'PIC SMM PASS', 0
fail_text: db 'PIC SMM FAIL', 0
expected:
db 0xa5,2,3,1,0x1c,3,0,1
db 1,0,1,0xa5,2,3,2,0xa5
db 1,0xfe,0,0,0xa5,0xa5,0xa5,0xa5
db 0xa5,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5,0xa5
times 1024-($-$$) db 0
