; Original MIT DOS device probe: NEC INT 1F/90 extended-memory round trip.
bits 16
cpu 8086
org 0x100
push ds
pop es
mov ax, ds
xor dx, dx
mov cx, 4
shift:
shl ax, 1
rcl dx, 1
loop shift
add ax, source
adc dx, 0
mov [gdt+0x12], ax
mov [gdt+0x14], dl
mov bx, gdt
mov cx, 16
xor si, si
xor di, di
mov ah, 0x90
int 0x1f
jc failed
mov ax, ds
xor dx, dx
mov cx, 4
shift2:
shl ax, 1
rcl dx, 1
loop shift2
add ax, result
adc dx, 0
mov [gdt+0x1a], ax
mov [gdt+0x1c], dl
mov word [gdt+0x12], 0
mov byte [gdt+0x14], 0x20
mov cx, 16
mov ah, 0x90
int 0x1f
jc failed
mov si, source
mov di, result
mov cx, 16
repe cmpsb
jne failed
mov dx, ok
mov ah, 9
int 0x21
mov ax, 0x4c00
int 0x21
failed:
mov dx, error
mov ah, 9
int 0x21
mov ax, 0x4c01
int 0x21
ok db 'EXTENDED MEMORY OK',13,10,'$'
error db 'EXTENDED MEMORY FAILED',13,10,'$'
source db 'PC98 INT1F RAM!!'
result times 16 db 0
align 2
gdt times 16 db 0
dw 0xffff, 0
db 0, 0x92, 0, 0
dw 0xffff, 0
db 0x20, 0x92, 0, 0
