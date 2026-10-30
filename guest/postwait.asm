; SPDX-License-Identifier: MIT
; Original Issue #113 post-handoff control, run by actual DOS from AUTOEXEC.
; Leave a persistent NUL background while polling IRQ-fed BIOS keyboard input.
; Argument M masks only IRQ1 for 64 IRQ0 ticks, then the guest restores the mask.
; This is an intentional negative control, not a model of the private HDD.
bits 16
cpu 8086
org 100h
push cs
pop ds
cmp byte [82h], 'M'
jne normal
mov byte [masked], 1
normal:
mov ah, 3
int 18h                         ; flush characters, never inject a result
in al, 2
mov [saved_mask], al
cmp byte [masked], 0
je screen
or al, 2
out 2, al                       ; IRQ0 remains enabled
screen:
cld
mov ax, 0a000h
mov es, ax
xor di, di
xor ax, ax
mov cx, 2048
rep stosw
mov di, 2000h
mov ax, 00e1h                   ; enabled white text attributes
mov cx, 2048
rep stosw
xor di, di
mov si, ready
mov cx, ready_end-ready
print_ready:
lodsb
xor ah, ah
stosw
loop print_ready
xor ax, ax
mov es, ax
mov ax, [es:058ah]
mov [start_tick], ax
mov byte [stage], 1
sti
jmp wait_key
ready: db 'POSTWAIT READY'
ready_end:
times 100h-($-$$) db 90h
; Fixed 0200h loop and 0300h state allow read-only native/Wasm observations
; relative to the *executing* COM segment, with no low-memory result overwrite.
wait_key:
inc word [polls]
mov ax, [es:058ah]
sub ax, [start_tick]
mov [elapsed], ax
cmp ax, 64
jb poll
mov al, [saved_mask]
out 2, al
poll:
mov ah, 5
int 18h                         ; nonblocking read, BH=1 only after real IRQ1
test bh, bh
jz wait_key
cmp al, 13
jne wait_key
mov [key], al
mov byte [stage], 2
mov al, [saved_mask]
out 2, al
mov dx, passed
mov ah, 9
int 21h
mov ax, 4c00h
int 21h
passed: db 13,10,'POSTWAIT ENTER PASS',13,10,'$'
times 200h-($-$$) db 0
signature: db 'H98STATE'
stage: db 0
masked: db 0
start_tick: dw 0
elapsed: dw 0
polls: dw 0
saved_mask: db 0
key: db 0
