; Original Ap uPD4990A Time Set probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/rtc_set.asm -o rtc_set.bin
; IPL at 1FC0:0000; results/guards 8000h..807Fh; stack below 7C00h.
; Every setting, delay and comparison is performed by guest instructions.
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
mov cx, 128
mov al, 0xa5
rep stosb
mov al, 0xff
out 2, al
out 0x0a, al

; Establish year 26 through guest ports on cold and retained-state boots.
mov si, epoch_date
call serial_set

; Parallel Jan 31 23:59:59 Sat: preserve the stored year (26 on both boots).
mov al, 1
call command
mov byte [control], 1
mov si, parallel_date
mov cx, 5
call write_bytes
mov al, 2
call command
mov bx, 0x8001
call bios_read
call held_delay
mov bx, 0x8007
call bios_read
mov al, 0
call command
mov bx, 0x8012
call await_zero
mov di, 0x800d
call parallel_read

; Extended 2028 Feb 28 23:59:59 Mon, complete 52-bit load with 0010 tail.
mov si, leap_date
call serial_set
mov bx, 0x8018
call bios_read
call held_delay
mov bx, 0x801e
call bios_read
mov al, 1
call serial_command
mov bx, 0x802b
call await_zero
mov di, 0x8024
call serial_read

; Extended year 99 -> 00; Time Read is the third release command.
mov si, rollover_date
call serial_set
mov bx, 0x8031
call bios_read
mov al, 3
call serial_command
mov bx, 0x803e
call await_zero
mov di, 0x8037
call serial_read

; Independently listed expected bytes, including both BIOS and serial orders.
mov si, expected
mov di, 0x8001
mov cx, 67
repe cmpsb
jne fail
cmp byte [es:0x8000], 0xa5
jne fail
mov di, 0x8044
mov cx, 60
mov al, 0xa5
repe scasb
jne fail
mov byte [es:0x8044], 0x5a
mov si, pass_text
call report
hlt
jmp $

serial_set:
mov al, 1
call serial_command
mov byte [control], 7
mov cx, 6
call write_bytes
; The command nibble belongs AFTER the six calendar bytes in the chain.
mov al, 2
jmp serial_command
write_bytes:
lodsb
mov dl, al
mov dh, 8
.bit:
mov al, dl
and al, 1
mov ah, 32
mul ah
or al, [control]
out 0x20, al
or al, 16
out 0x20, al
and al, 0xef
out 0x20, al
shr dl, 1
dec dh
jnz .bit
loop write_bytes
ret

held_delay:
; >1,100,000 LOOP slots (>one guest second) with IF clear. Held CLK activity
; must neither advance the date nor shift its seconds LSB (59h -> DATA OUT=1).
mov dx, 17
.outer:
mov cx, 0xffff
.inner:
loop .inner
dec dx
jnz .outer
mov cx, 12
.pulse:
mov al, 7
out 0x20, al
or al, 16
out 0x20, al
and al, 0xef
out 0x20, al
in al, 0x33
cmp al, 0x39
jne fail
loop .pulse
ret

await_zero:
call bios_read
cmp byte [es:bx+5], 0
jne await_zero
ret
bios_read:
mov ax, 0x00ab
int 0x1c
cmp ax, 0x00ab
jne fail
ret
parallel_read:
mov al, 3
call command
mov al, 1
call command
mov byte [control], 1
mov cx, 5
jmp read_bytes
serial_read:
mov al, 3
call serial_command
mov al, 1
call serial_command
mov byte [control], 7
mov cx, 6
call read_bytes
mov bh, 4
call read_bits
mov cl, 4
shr bl, cl
mov al, bl
stosb
ret
read_bytes:
mov bh, 8
call read_bits
mov al, bl
stosb
loop read_bytes
ret
read_bits:
xor bl, bl
.bit:
in al, 0x33
mov ah, al
and ah, 0xfe
cmp ah, 0x38
jne fail
shr al, 1
rcr bl, 1
mov al, [control]
or al, 16
out 0x20, al
and al, 0xef
out 0x20, al
dec bh
jnz .bit
ret
command:
out 0x20, al
or al, 8
out 0x20, al
and al, 0xf7
out 0x20, al
ret
serial_command:
mov dl, al
mov cx, 4
.bit:
mov al, dl
and al, 1
mov ah, 32
mul ah
or al, 7
out 0x20, al
or al, 16
out 0x20, al
and al, 0xef
out 0x20, al
shr dl, 1
loop .bit
mov al, 7
jmp command
fail:
cli
mov si, fail_text
call report
hlt
jmp fail
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
ret
control: db 0
epoch_date: db 0,0,0,1,0x14,0x26
parallel_date: db 0x59,0x59,0x23,0x31,0x16
leap_date: db 0x59,0x59,0x23,0x28,0x21,0x28
rollover_date: db 0x59,0x59,0x23,0x31,0xc5,0x99
expected:
db 0x26,0x16,0x31,0x23,0x59,0x59 ; parallel BIOS
db 0x26,0x16,0x31,0x23,0x59,0x59 ; held BIOS
db 0,0,0,1,0x20                  ; next month serial
db 0x26,0x20,1,0,0,0             ; next month BIOS
db 0x28,0x21,0x28,0x23,0x59,0x59 ; extended BIOS
db 0x28,0x21,0x28,0x23,0x59,0x59 ; held BIOS
db 0,0,0,0x29,0x22,0x28,1        ; leap day serial + command
db 0x28,0x22,0x29,0,0,0          ; leap day BIOS
db 0x99,0xc5,0x31,0x23,0x59,0x59 ; rollover BIOS
db 0,0,0,1,0x16,0,1              ; year 00 serial + command
db 0,0x16,1,0,0,0                ; year 00 BIOS
pass_text: db 'RTC SET PASS',0
fail_text: db 'RTC SET FAIL',0
times 1024-($-$$) db 0
