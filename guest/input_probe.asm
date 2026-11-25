; MIT regression probe for BIOS keyboard, FDC, text/bitmap VRAM, beeper and bus mouse.
bits 16
cpu 8086
org 0

cli
xor ax, ax
mov ss, ax
mov sp, 0x7c00
cld
push cs
pop ds
mov ax, 0x2000
mov es, ax
; Boot via sector 1, then read the remaining track, including the writeback sector.
mov bp, 0x1000
mov bx, 7 * 1024
mov cx, 0x0300
mov dx, 2
mov ax, 0x5690
int 0x1b
jc failed
mov ax, [es:0x2800]
mov [saved_keys], ax
mov ax, 0xa000
mov es, ax
xor di, di
mov si, title
.print:
lodsb
test al, al
jz .sound
xor ah, ah
stosw
jmp .print
.sound:
; Use the hardware channel feeding the beeper, then turn on its gate.
mov al, 0x76
out 0x77, al
mov ax, 0x5000
out 0x73, al
mov al, ah
out 0x73, al
mov ah, 0x17
int 0x18
mov byte [ready], 1
sti

.wait:
hlt
call mouse
mov ah, 1
int 0x18
test bh, bh
jz .wait
xor ah, ah
int 0x18
mov [last_key], ah
inc word [keys]
mov ax, 0xa000
mov es, ax
mov al, [keys]
and al, 15
add al, 'A'
mov [es:78], al
mov ax, 0xa800
mov es, ax
xor byte [es:1600], 0xff
mov ax, 0x2000
mov es, ax
mov ax, [keys]
mov [es:0x2800], ax
mov bp, 0x2800
mov bx, 1024
mov cx, 0x0300
mov dx, 8
mov ax, 0x5590
int 0x1b
jnc .wait
failed:
cli
hlt
jmp failed

mouse:
mov dx, 0x7fdd
in al, dx
test al, 8
jz .done
xor al, al
out dx, al
mov al, 0x80
call .sample
mov ah, bl
and ah, 0xe0
xor ah, 0xe0
cmp ah, [buttons]
je .same
mov [buttons], ah
inc word [edges]
.same:
cbw
add [total_x], ax
mov al, 0xc0
call .sample
cbw
add [total_y], ax
.done:
ret
.sample:
mov dx, 0x7fdd
out dx, al
mov bh, al
mov dx, 0x7fd9
in al, dx
mov bl, al
mov dx, 0x7fdd
mov al, bh
or al, 0x20
out dx, al
mov dx, 0x7fd9
in al, dx
mov cl, 4
shl al, cl
mov ah, bl
and ah, 15
or al, ah
ret

title db 'INPUT PROBE READY', 0
times 0x300-($-$$) db 0
ready db 0
last_key db 0xff
keys dw 0
saved_keys dw 0
total_x dw 0
total_y dw 0
buttons db 0
edges dw 0
times 1024-($-$$) db 0
