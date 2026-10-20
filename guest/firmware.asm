; Original synthetic PC-98 IPL. MIT. ROM segment F000, entry F000:0000.
; INT vectors point at four-byte OUT F0 / IRET service thunks (see firmware.rs).
bits 16
cpu 8086
org 0
cli
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov ds, ax
mov es, ax
cld
; Program the two 8259s with PC-98 vectors and cascade on master IR7.
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x10
out 0x0a, al
mov al, 0x80
out 2, al
mov al, 7
out 0x0a, al
mov al, 1
out 2, al
out 0x0a, al
mov al, 0x7c ; timer + keyboard + cascade
out 2, al
mov al, 0xf7 ; 2HD FDC IR11
out 0x0a, al
; 100 Hz periodic IRQ0 at the 8 MHz VM PIT clock.
mov al, 0x36
out 0x77, al
mov ax, 19968
out 0x71, al
mov al, ah
out 0x71, al
mov al, 0x76
out 0x77, al
mov ax, 1997
out 0x73, al
mov al, ah
out 0x73, al
; Text GDC and slave bitmap GDC start display.
mov al, 0x0d
out 0x62, al
out 0xa2, al
; BIOS read: drive 90, C=0 H=0 R=1 N=3, bytes=1024 to 1FC0:0000.
mov ax, 0x1fc0
mov es, ax
xor bp, bp
mov bx, 1024
mov cx, 0x0300
mov dx, 1
mov ax, 0x5690
int 0x1b
jc failed
sti
jmp 0x1fc0:0
failed:
int 0x1e
hlt
jmp failed
