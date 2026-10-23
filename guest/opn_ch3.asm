; Original channel-3 frequency selection probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/opn_ch3.asm -o opn_ch3.bin
; One 1024-byte IPL sector at 1FC0:0000. No DOS, ROM or sample dependencies.
; 8001h identifies stable PCM stages 1..16; 0=between, FF=done.
; For each logical operator: normal, 40h, 80h, normal WHILE KEYED.
; Each stage runs 80000 LOOP instructions. Entire boot <2000000 slots.
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
mov cx, 64
mov al, 0xa5
rep stosb
mov byte [es:0x8001], 0
mov al, 0xff
out 2, al
out 0x0a, al
; Every operator: no detune, multiplier one, TL zero, immediate attack,
; no decay/sustain attenuation, fastest release; algorithm 7, no feedback.
mov si, registers
.init:
lodsw
cmp al, 0xff
je .initialized
call fm_write
jmp .init
.initialized:
xor bx, bx
mov bp, 1
.operator:
mov al, [keys+bx]
mov ah, al
mov al, 0x28
call fm_write
mov si, modes
.mode:
lodsb
mov ah, al
mov al, 0x27
call fm_write                    ; timers remain stopped, no key writes here
mov ax, bp
mov [es:0x8001], al
mov cx, 40000
.delay1:
loop .delay1
mov cx, 40000
.delay2:
loop .delay2
mov byte [es:0x8001], 0
mov di, bp
mov byte [es:di+0x800f], 1       ; stage completion, 8010h..801Fh
inc bp
mov ax, bp
dec ax
and ax, 3
jnz .mode
mov ax, 0x0228                  ; key off, release before next operator
call fm_write
mov cx, 8000
.release:
loop .release
inc bx
cmp bx, 4
jb .operator
mov byte [es:0x8001], 0xff
mov si, expected
mov di, 0x8000
mov cx, 64
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

; AL=register, AH=value. High frequency bytes precede low bytes in table.
fm_write:
push ax
push dx
mov dx, 0x188
out dx, al
add dx, 2
mov al, ah
out dx, al
pop dx
pop ax
ret
fail:
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
keys: db 0x12,0x22,0x42,0x82
modes: db 0,0x40,0x80,0
registers:
db 0x27,0, 7,0xff, 0xb2,7
%assign offset 2
%rep 4
db 0x30+offset,1, 0x40+offset,0, 0x50+offset,31
db 0x60+offset,0, 0x70+offset,0, 0x80+offset,15
%assign offset offset+4
%endrep
; Logical operator mapping 1=A9/AD, 2=AA/AE, 3=A8/AC, 4=A2/A6.
; Distinct 11-bit F-numbers and blocks catch physical/logical slot swaps.
db 0xad,0x19, 0xa9,0x23        ; op1: block 3, FNUM 123h
db 0xae,0x24, 0xaa,0x56        ; op2: block 4, FNUM 456h
db 0xac,0x17, 0xa8,0x89        ; op3: block 2, FNUM 789h
db 0xa6,0x2d, 0xa2,0xab        ; op4/normal: block 5, FNUM 5ABh
db 0xff,0
pass_text: db 'OPN CH3 PASS',0
fail_text: db 'OPN CH3 FAIL',0
expected:
db 0xa5,0xff
times 14 db 0xa5
times 16 db 1
times 32 db 0xa5
times 1024-($-$$) db 0
