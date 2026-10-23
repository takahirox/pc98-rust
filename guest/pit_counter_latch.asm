; Original ordinary 8253 counter-latch retention probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pit_counter_latch.asm -o pit_counter_latch.bin
; One 1024-byte IPL sector loaded at 1FC0:0000. Guest owns 8000h..801Fh.
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
mov al, 0xff
out 2, al
out 0x0a, al

; Case 1: multiple duplicate commands before either byte has been read.
call program
xor al, al
out 0x77, al
call delay
mov dx, 0x3fdf                ; duplicate through the other control bank
xor al, al
out dx, al
out 0x77, al
mov dx, 0x3fd9
in al, dx
mov [es:0x8001], al
mov ah, al
in al, 0x71
mov [es:0x8002], al
xchg al, ah
cmp ah, 0x12                  ; replacement would have high byte 10h
jne fail
mov bx, ax
call recapture
mov [es:0x8003], ax
call check_fresh

; Case 2: retain the next unread high byte, even across several duplicates.
call program
mov dx, 0x3fdf
xor al, al
out dx, al
in al, 0x71
mov [es:0x8005], al
call delay
xor al, al
out 0x77, al
mov dx, 0x3fdf
out dx, al
mov dx, 0x3fd9
in al, dx
mov [es:0x8006], al
cmp al, 0x12                  ; a restarted low byte must not masquerade as high
jne fail
mov bx, [es:0x8005]
call recapture
mov [es:0x8007], ax
call check_fresh

; Compare all raw guest observations and every guard before reporting PASS.
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

program:
mov al, 0x30                  ; paired binary mode 0, counter 0
out 0x77, al
mov ax, 0x1234
out 0x71, al
mov al, ah
out 0x71, al
mov cx, 16                    ; allow load, leave byte-distinct 12xxh count
.settle:
loop .settle
ret

delay:
mov cx, 200                   ; >256 clocks, far short of terminal count
.wait:
loop .wait
ret

recapture:
mov dx, 0x3fdf
xor al, al
out dx, al                    ; original snapshot is now fully consumed
in al, 0x71
mov ah, al
mov dx, 0x3fd9
in al, dx
xchg al, ah
ret

check_fresh:
cmp ax, 0x0e00
jb fail
cmp ax, 0x1100
jae fail
sub bx, ax
cmp bx, 0x100                 ; must capture the running count, not stale data
jb fail
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
pass_text: db 'PIT COUNTER LATCH PASS', 0
fail_text: db 'PIT COUNTER LATCH FAIL', 0
expected:
db 0xa5, 0x0d,0x12,0x54,0x10,0x0b,0x12,0x56,0x10
times 23 db 0xa5
times 1024-($-$$) db 0
