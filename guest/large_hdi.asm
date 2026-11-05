; Original MIT large-HDI guest probe. No external firmware or emulator code.
; Booted through the real synthetic PC-98 IPL and IDE. Host only steps/reads.
bits 16
cpu 386
org 0
cli
mov ax, cs
mov es, ax
mov bp, 512
mov bx, 15*512
xor cx, cx
mov dx, 1
mov ax, 0x0680
int 0x1b
jc failed
jmp main
failed:
push cs
pop ds
mov si, fail_text
jmp report
times 510-($-$$) db 0
dw 0xaa55
times 512-($-$$) db 0
main:
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
push cs
pop ds
push cs
pop es
; BIOS CHS geometry and IDENTIFY capacity must agree, including high word.
mov ax, 0x8480
int 0x1b
jc failed
cmp bx, 512
jne failed
cmp dx, 0x1020
jne failed
cmp cx, 2048
je .geometry
cmp cx, 4096
jne failed
.geometry:
mov [cylinders], cx
movzx eax, cx
shl eax, 9
mov [capacity], eax
mov ebx, eax
shr eax, 1
dec eax
mov [addresses+8], eax
sub ebx, 2
mov [addresses+12], ebx
mov dx, 0x64e
mov al, 0xec
out dx, al
mov dx, 0x640
mov di, buffer
mov cx, 256
rep insw
mov eax, [buffer+120]
cmp eax, [capacity]
jne failed
mov ax, [buffer+2]
cmp ax, [cylinders]
jne failed
; BIOS first physical sector retains the guest IPL signature.
xor cx, cx
xor dx, dx
mov bx, 512
mov bp, buffer
mov ax, 0x0600
int 0x1b
jc failed
cmp word [buffer+510], 0xaa55
jne failed
; BIOS high last-sector read, before our write: zero on cold mount, 5A after
; reset/export/reimport. Any other value or truncation fails in guest code.
mov eax, [capacity]
dec eax
mov ecx, eax
shr eax, 16
mov dx, ax
mov bx, 512
mov bp, buffer
mov ax, 0x0600
int 0x1b
jc failed
cmp word [buffer], 0
je .fresh
cmp word [buffer], 0x5a5a
jne failed
mov byte [persisted], 1
.fresh:
mov si, addresses
mov bp, 4
.next:
mov eax, [si]
call taskfile
mov al, 0x30
out dx, al
mov dx, 0x640
mov ax, 0x5a5a
mov cx, 512                  ; two sectors, including last physical sector
.write:
out dx, ax
loop .write
mov dx, 0x64e
in al, dx
and al, 9
jnz failed
mov eax, [si]
call taskfile
mov al, 0x20
out dx, al
mov dx, 0x640
mov cx, 512
.read:
in ax, dx
cmp ax, 0x5a5a
jne failed
loop .read
mov dx, 0x644
in al, dx
or al, al
jnz failed
add si, 4
 dec bp
jnz .next
; CHS last physical sector (one-based IDE, zero-based BIOS).
mov dx, 0x644
mov al, 1
out dx, al
add dx, 2
mov al, 32
out dx, al
add dx, 2
mov ax, [cylinders]
dec ax
out dx, al
add dx, 2
mov al, ah
out dx, al
mov dx, 0x64c
mov al, 0xaf
out dx, al
mov dx, 0x64e
mov al, 0x20
out dx, al
mov dx, 0x640
mov cx, 256
.chs:
in ax, dx
cmp ax, 0x5a5a
jne failed
loop .chs
; BIOS high CHS read and write: zero-based sector 31, head 15.
mov cx, [cylinders]
dec cx
mov dx, 0x0f1f
mov bp, buffer
mov bx, 512
mov ax, 0x0680
int 0x1b
jc failed
cmp word [buffer], 0x5a5a
jne failed
mov ax, 0x0580
int 0x1b
jc failed
; Entire out-of-range two-sector WRITE must reject without partial damage.
mov eax, [capacity]
dec eax
call taskfile
mov al, 0x30
out dx, al
in al, dx
and al, 9
cmp al, 1
jne failed
mov dx, 0x642
in al, dx
cmp al, 0x10
jne failed
; BIOS full 32-bit out-of-range LBA must reject, not wrap to IDE 28 bits.
mov cx, 0
mov dx, 0x1000
mov bx, 512
mov ax, 0x0600
int 0x1b
jnc failed
; Maximal 32-bit LBA plus one sector must fail without host integer overflow.
mov cx, 0xffff
mov dx, 0xffff
mov bx, 512
mov ax, 0x0600
int 0x1b
jnc failed
mov si, pass_text
cmp byte [persisted], 0
je report
mov si, persist_text
report:
mov ax, 0xa000
mov es, ax
xor di, di
.print:
lodsb
or al, al
jz .halt
xor ah, ah
stosw
jmp .print
.halt:
cli
hlt
jmp .halt
; EAX LBA, two sectors, nIEN avoids unhandled external interrupts.
taskfile:
push eax
mov dx, 0x74c
mov al, 2
out dx, al
mov dx, 0x644
mov al, 2
out dx, al
pop eax
mov dx, 0x646
out dx, al
shr eax, 8
add dx, 2
out dx, al
shr eax, 8
add dx, 2
out dx, al
shr eax, 8
and al, 15
or al, 0xe0
add dx, 2
out dx, al
add dx, 2
ret
cylinders dw 0
capacity dd 0
persisted db 0
addresses dd 119, 65535, 0, 0
pass_text db 'LARGE HDI PASS', 13, 10, 0
persist_text db 'LARGE HDI PERSIST LARGE HDI PASS', 0
fail_text db 'LARGE HDI FAIL', 13, 10, 0
align 2
buffer times 512 db 0
times 8192-($-$$) db 0
