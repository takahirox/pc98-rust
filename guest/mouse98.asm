; Original MIT resident PC-98 bus mouse driver. DOS INT 33h subset for ADV98.
; Real 7FD9/7FDD/7FDF ports and IRQ13 (INT 15h); no host mouse BIOS shortcuts.
bits 16
cpu 8086
org 0x100
jmp install
db 'M98STATE'
x dw 320
y dw 200
buttons dw 0
ticks dw 0
motion_x dw 0
motion_y dw 0
min_x dw 0
max_x dw 639
min_y dw 0
max_y dw 399

api:
push ds
push cs
pop ds
cmp ax, 0
je reset_mouse
cmp ax, 1
je done
cmp ax, 2
je done
cmp ax, 3
je position
cmp ax, 4
je set_position
cmp ax, 7
je range_x
cmp ax, 8
je range_y
cmp ax, 0x0b
je relative
xor ax, ax
done:
pop ds
iret
reset_mouse:
mov word [x], 320
mov word [y], 200
mov word [motion_x], 0
mov word [motion_y], 0
mov ax, 0xffff
mov bx, 2
jmp done
position:
mov cx, [x]
mov dx, [y]
mov bx, [buttons]
jmp done
set_position:
mov [x], cx
mov [y], dx
jmp done
range_x:
mov [min_x], cx
mov [max_x], dx
jmp done
range_y:
mov [min_y], cx
mov [max_y], dx
jmp done
relative:
mov cx, [motion_x]
mov dx, [motion_y]
mov word [motion_x], 0
mov word [motion_y], 0
jmp done

irq:
push ax
push bx
push cx
push dx
push ds
push cs
pop ds
inc word [ticks]
mov dx, 0x7fdd
xor al, al
out dx, al
mov al, 0x80
out dx, al
mov dx, 0x7fd9
in al, dx
mov bl, al
mov cl, 5
shr al, cl
not al
and ax, 5
mov cx, ax
and ax, 4
shr ax, 1
shr ax, 1
and cx, 1
shl cx, 1
or ax, cx
mov [buttons], ax
and bl, 15
mov dx, 0x7fdd
mov al, 0xa0
out dx, al
mov dx, 0x7fd9
in al, dx
mov cl, 4
shl al, cl
or al, bl
cbw
add [motion_x], ax
add ax, [x]
cmp ax, [min_x]
jge x_lower
mov ax, [min_x]
x_lower:
cmp ax, [max_x]
jle x_upper
mov ax, [max_x]
x_upper:
mov [x], ax
mov dx, 0x7fdd
mov al, 0xc0
out dx, al
mov dx, 0x7fd9
in al, dx
and al, 15
mov bl, al
mov dx, 0x7fdd
mov al, 0xe0
out dx, al
mov dx, 0x7fd9
in al, dx
shl al, cl
or al, bl
cbw
add [motion_y], ax
add ax, [y]
cmp ax, [min_y]
jge y_lower
mov ax, [min_y]
y_lower:
cmp ax, [max_y]
jle y_upper
mov ax, [max_y]
y_upper:
mov [y], ax
mov dx, 0x7fdd
xor al, al
out dx, al
mov al, 0x20
out 8, al
out 0, al
pop ds
pop dx
pop cx
pop bx
pop ax
iret
resident_end:

install:
mov ax, 0x2533
mov dx, api
int 0x21
mov ax, 0x2515
mov dx, irq
int 0x21
mov dx, 0x7fdf
mov al, 0x93
out dx, al
mov dx, 0xbfdb
xor al, al
out dx, al
in al, 0x0a
and al, 0xdf
out 0x0a, al
mov dx, (resident_end - $$ + 0x100 + 15) / 16
mov ax, 0x3100
int 0x21
