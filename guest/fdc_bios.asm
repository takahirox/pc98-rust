; Original BIOS/FDC mode handoff probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/fdc_bios.asm -o fdc_bios.bin
; Synthetic IPL loads this single 1024-byte sector at 1FC0:0000.
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
mov cx, 0x4410
mov al, 0xa5
rep stosb
mov al, 0xff
out 2, al
out 10, al                      ; poll PIO; no host IRQ assistance
xor al, al
out 0x11, al                    ; enable DMA controller
mov al, 0x18
out 0x94, al
; Independent ND=1 -> BIOS reads, same cylinder and seek required.
call nd
mov ax, 0x0690
mov cx, 0x0300
mov dx, 2
mov bp, 0x9000
call bios
call nd
mov ax, 0x0690
mov cx, 0x0302
mov dx, 2
mov bp, 0x9800
call bios
; Distinct original write payloads, each preceded by fresh ND=1.
mov di, 0xa000
mov al, 71
mov bl, 29
call fill
mov di, 0xa800
mov al, 0xb3
mov bl, 13
call fill
call nd
mov ax, 0x0590
mov cx, 0x0300
mov dx, 3
mov bp, 0xa000
call bios
call nd
mov ax, 0x0590
mov cx, 0x0302
mov dx, 3
mov bp, 0xa800
call bios
; Read back both intended writes through BIOS and compare every byte.
mov ax, 0x0690
mov cx, 0x0300
mov dx, 3
mov bp, 0xb000
call bios
mov di, 0xb000
mov al, 71
mov bl, 29
call linear
mov ax, 0x0690
mov cx, 0x0302
mov dx, 3
mov bp, 0xb000
call bios
mov di, 0xb000
mov al, 0xb3
mov bl, 13
call linear
; Completed PIO -> BIOS (including seek) -> fresh ND=1 -> PIO.
call nd
mov si, read_c2
mov di, 0xb800
call pio
mov ax, 0x0690
mov cx, 0x0300
mov dx, 2
mov bp, 0x9000
call bios
call nd
mov si, read_c0
mov di, 0xc000
call pio
; Independent payload oracles: C0/R2=(i xor (i>>8)); C2/R2=7*i+31h.
mov di, 0x9000
call xor_pattern
mov di, 0xc000
call xor_pattern
mov di, 0x9800
mov al, 0x31
mov bl, 7
call linear
mov di, 0xb800
mov al, 0x31
mov bl, 7
call linear
mov di, 0xa000
mov al, 71
mov bl, 29
call linear
mov di, 0xa800
mov al, 0xb3
mov bl, 13
call linear
; All surrounding RAM, including record padding, is guarded.
mov si, guard_ranges
mov bx, 8
.check_guards:
lodsw
mov di, ax
lodsw
mov cx, ax
.guard:
cmp byte [es:di], 0xa5
jne fail
inc di
loop .guard
dec bx
jnz .check_guards
cmp sp, 0x7c00
jne fail
mov byte [es:0x8000], 1
mov si, pass_text
jmp report
bios:
mov bx, 1024
int 0x1b
pushf
pop dx
mov di, [record]
mov [es:di], ah
and dl, 1
mov [es:di+1], dl
add word [record], 2
test dl, dl
jnz fail
test ah, ah
jnz fail
call idle
ret
nd:
mov si, specify
mov cx, 3
call command
call idle
ret
command:
in al, 0x90
and al, 0xc0
cmp al, 0x80
jne command
lodsb
out 0x92, al
loop command
ret
idle:
in al, 0x90
cmp al, 0x80
jne idle
ret
fill:
mov cx, 1024
.fill:
stosb
add al, bl
loop .fill
ret
linear:
mov cx, 1024
.check:
cmp al, [es:di]
jne fail
inc di
add al, bl
loop .check
ret
xor_pattern:
xor bx, bx
.check:
mov ax, bx
xor al, ah
cmp al, [es:di]
jne fail
inc di
inc bx
cmp bx, 1024
jb .check
ret
pio:
mov al, [si+2]                 ; expected result cylinder, not a host observation
mov [result+3], al
mov cx, 9
call command
mov cx, 1024
.data:
in al, 0x90
test al, 0x80
jz .data
cmp al, 0xf0                    ; EXM must distinguish data from results
jne fail
in al, 0x92
stosb
loop .data
.wait_result:
in al, 0x90
test al, 0x80
jz .wait_result
cmp al, 0xd0
jne fail
mov si, result
mov cx, 7
.result:
in al, 0x90
cmp al, 0xd0
jne fail
in al, 0x92
cmp al, [si]
jne fail
inc si
loop .result
call idle
ret
fail:
cli
mov si, fail_text
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
hlt
jmp .done
record: dw 0x8001
specify: db 3,0xdf,3
read_c2: db 0x46,0,2,0,2,3,2,0x1b,0xff
read_c0: db 0x46,0,0,0,2,3,2,0x1b,0xff
result: db 0x40,0x80,0,0,0,2,3
guard_ranges:
dw 0x800f,0x0ff1, 0x9400,0x400, 0x9c00,0x400, 0xa400,0x400
dw 0xac00,0x400, 0xb400,0x400, 0xbc00,0x400, 0xc400,0x10
pass_text: db 'FDC BIOS PASS',0
fail_text: db 'FDC BIOS FAIL',0
times 1024-($-$$) db 0
