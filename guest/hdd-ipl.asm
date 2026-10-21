; Original MIT PC-98 partition IPL, loaded at 1FC0:0 by the synthetic BIOS.
; Select the first active DOS partition, read its two-sector IPL and jump to it.
bits 16
cpu 8086
org 0
push cs
pop ds
xor ax, ax
mov es, ax
mov bp, 0x800
mov bx, 512
mov cx, 1
xor dx, dx
mov ax, 0x0600
int 0x1b
jc failed
xor si, si
find_partition:
test byte [es:si+0x800], 0x80
jz next
cmp byte [es:si+0x801], 0x81
je found
next:
add si, 32
cmp si, 512
jb find_partition
failed:
int 0x1e
hlt
jmp failed
found:
mov cx, [es:si+0x806]
mov dx, [es:si+0x804]
mov ax, 0x1fe0
mov es, ax
xor bp, bp
mov bx, 1024
mov ax, 0x0680
int 0x1b
jc failed
; FreeDOS preserves SI as the partition index at 0000:03FE.
mov cl, 5
shr si, cl
jmp 0x1fe0:0
times 254-($-$$) db 0
dw 0xaa55 ; PC-98 signature, also valid on 256-byte-sector IPLs
times 510-($-$$) db 0
dw 0xaa55
