; Original bare-metal IDE READ VERIFY probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/ide_verify.asm -o ide_verify.bin
; Original synthetic IPL loads sector zero at 1FC0:0000. Load the rest via
; INT 1B (zero-based HDD CHS), then issue every VERIFY through actual ports.
bits 16
cpu 8086
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
jc boot_fail
jmp main
boot_fail:
push cs
pop ds
mov si, fail_text
jmp report
times 512-($-$$) db 0
main:
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
mov di, 0x8000
mov cx, 1024
mov al, 0xa5
rep stosb
mov di, 0x8010
; Firmware polls with nIEN. Keep it suppressed throughout the result matrix.
mov dx, 0x74c
mov al, 2
out dx, al
mov si, cases
mov bp, (cases_end-cases)/13
.next:
mov dx, 0x644
mov cx, 5
.program:
lodsb
out dx, al
add dx, 2
loop .program
lodsb                         ; opcode at 64E
out dx, al
mov dx, 0x644
mov cx, 5
.read:
in al, dx
stosb
cmp al, [si]
jne fail
inc si
add dx, 2
loop .read
mov dx, 0x642
in al, dx
stosb
cmp al, [si]
jne fail
inc si
mov dx, 0x74c                 ; alternate status preserves device latch
in al, dx
stosb
cmp al, [si]
jne fail
inc si
mov dx, 0x64e
in al, dx
cmp al, [si-1]
jne fail
mov dx, 0x640
in al, dx
stosb
cmp al, 0xff                 ; non-data, DRQ clear
jne fail
mov al, 0x5a
out dx, al                    ; must not write media
inc dx
out dx, al
dec bp
jnz .next

; All CPU interrupts stay disabled. Observe the slave PIC separately from the
; device latch: status acknowledgement cannot remove a previously latched IR1.
mov al, 0xfd
out 0x0a, al
mov dx, 0x74c
xor al, al
out dx, al
call issue
call irr
mov [es:0x8001], al
cmp al, 2
jne fail
mov dx, 0x74c
in al, dx
cmp al, 0x50
jne fail
call drain                    ; PIC acknowledgement while device latch persists
call irr                      ; alternate status must allow IR1 to relatch
mov [es:0x8002], al
cmp al, 2
jne fail
mov dx, 0x64e
in al, dx
cmp al, 0x50
jne fail
call irr
mov [es:0x8003], al
cmp al, 2                     ; primary status clears device, not PIC
jne fail
call drain
call irr
mov [es:0x8004], al
cmp al, 0
jne fail
mov dx, 0x74c
mov al, 2
out dx, al
call issue
call irr
mov [es:0x8005], al
cmp al, 0                     ; nIEN suppresses delivery
jne fail
mov dx, 0x74c
in al, dx
cmp al, 0x50
jne fail
xor al, al
out dx, al
call irr
mov [es:0x8006], al
cmp al, 2                     ; enabling exposes the retained device latch
jne fail
mov dx, 0x64e
in al, dx
call drain
call irr
mov [es:0x8007], al
cmp al, 0
jne fail
; A subsequent valid IDENTIFY, then READ: guest checks geometry and reads all
; data without altering media. Native tests separately exercise WRITE recovery.
mov dx, 0x64e
mov al, 0xec
out dx, al
mov dx, 0x74c
in al, dx
cmp al, 0x58
jne fail
mov dx, 0x640
in ax, dx
cmp ax, 0x40
jne fail
in ax, dx
cmp ax, 4
jne fail
in ax, dx
in ax, dx
cmp ax, 4
jne fail
mov cx, 252
.id:
in ax, dx
loop .id
call issue                    ; returns to a valid non-data command after IDENTIFY
mov dx, 0x644
mov al, 1
out dx, al
add dx, 2
mov al, 31                    ; untouched synthetic sector, filled with 0x5a
out dx, al
add dx, 2
xor al, al
out dx, al
add dx, 2
out dx, al
add dx, 2
mov al, 0xe0
out dx, al
add dx, 2
mov al, 0x20
out dx, al
mov dx, 0x74c
in al, dx
cmp al, 0x58
jne fail
mov dx, 0x640
mov cx, 256
.data:
in ax, dx
cmp ax, 0x5a5a
jne fail
loop .data
mov dx, 0x64e
in al, dx
cmp al, 0x50
jne fail
mov byte [es:0x8008], 1
; Check all guards, including the entire unused result tail, before PASS.
mov bx, 0x8000
call guard
mov bx, 0x8009
mov cx, 7
.guard_prefix:
call guard
inc bx
loop .guard_prefix
mov bx, di
mov cx, 0x8400
sub cx, bx
.guard_tail:
call guard
inc bx
loop .guard_tail
cmp sp, 0x7c00
jne fail
mov si, pass_text
jmp report

guard:
cmp byte [es:bx], 0xa5
jne fail
ret
issue:
mov dx, 0x644
mov al, 1
out dx, al
add dx, 2
mov al, 7
out dx, al
add dx, 2
xor al, al
out dx, al
add dx, 2
out dx, al
add dx, 2
mov al, 0xe0
out dx, al
add dx, 2
mov al, 0x40
out dx, al
ret
irr:
mov al, 0x0a
out 8, al
in al, 8
and al, 2
ret
drain:
mov al, 0x0c
out 8, al
in al, 8
cmp al, 0x81
jne fail
mov al, 0x20
out 8, al
ret
fail:
mov si, fail_text
report:
cli
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
hlt
jmp .halt
pass_text: db 'IDE VERIFY PASS', 0
fail_text: db 'IDE VERIFY FAIL', 0

; Literal expected task-file bytes, independently specified for this geometry.
; Input: count,S,CL,CH,DH,opcode. Output: count,S,CL,CH,DH,error,status.
%macro V 12
 db %1,%2,%3,%4,%5,OP,%6,%7,%8,%9,%10,%11,%12
%endmacro
cases:
%assign OP 0x40
%rep 2
; LBA counts 1,2,255,256; byte carry, exact final-sector success, both errors.
V 1,7,0,0,0xe0,     0,7,0,0,0xe0,0,0x50
V 2,255,0,0,0xe0,   0,0,1,0,0xe0,0,0x50
V 255,7,0,0,0xe0,   0,5,1,0,0xe0,0,0x50
V 0,7,0,0,0xe0,     0,6,1,0,0xe0,0,0x50
V 2,254,1,0,0xe0,   0,255,1,0,0xe0,0,0x50
V 4,254,1,0,0xe0,   2,0,2,0,0xe0,0x10,0x51
V 0,254,1,0,0xe0,   254,0,2,0,0xe0,0x10,0x51
V 0,0,2,0,0xe0,     0,0,2,0,0xe0,0x10,0x51
V 2,0,0,1,0xe0,     2,0,0,1,0xe0,0x10,0x51
V 1,7,0,0,0xe0,     0,7,0,0,0xe0,0,0x50
; CHS head/cylinder transitions and the same counts, boundary and error cases.
V 1,8,0,0,0xa0,     0,8,0,0,0xa0,0,0x50
V 2,32,0,0,0xa0,    0,1,0,0,0xa1,0,0x50
V 2,32,0,0,0xa3,    0,1,1,0,0xa0,0,0x50
V 255,8,0,0,0xa0,   0,6,2,0,0xa0,0,0x50
V 0,8,0,0,0xa0,     0,7,2,0,0xa0,0,0x50
V 2,31,3,0,0xa3,    0,32,3,0,0xa3,0,0x50
V 4,31,3,0,0xa3,    2,1,4,0,0xa0,0x10,0x51
V 0,31,3,0,0xa3,    254,1,4,0,0xa0,0x10,0x51
V 1,0,0,0,0xa0,     1,0,0,0,0xa0,0x10,0x51
V 2,33,0,0,0xa0,    2,33,0,0,0xa0,0x10,0x51
V 255,1,0,0,0xa4,   255,1,0,0,0xa4,0x10,0x51
V 0,1,4,0,0xa0,     0,1,4,0,0xa0,0x10,0x51
V 1,1,0,0,0xa0,     0,1,0,0,0xa0,0,0x50
%assign OP OP+1
%endrep
cases_end:
times 8192-($-$$) db 0
