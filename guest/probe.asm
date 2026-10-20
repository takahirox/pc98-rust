; Original MIT licensed DOS executable. DOS INT 21 is handled by the guest OS.
bits 16
cpu 8086
org 0x100
push cs
pop ds
mov dx, message
mov ah, 9
int 0x21
mov ah, 0x17
int 0x18
mov ax, 0x4c00
int 0x21
message db 13,10,'PC98 DOS PROBE OK',13,10,'$'
