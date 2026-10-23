; Original Ap uPD4990A serial calendar probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/rtc.asm -o rtc.bin
; IPL at 1FC0:0000; result and guards at 8000h..803Fh, stack below 7C00h.
; Cold epoch 2026-01-01 Thu 00:00:00; reset retains the RTC, so boot 2
; observes seconds 01 -> 02. All advancement comes from executed instructions.
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
mov cx, 64
mov al, 0xa5
rep stosb
mov al, 0xff
out 2, al
out 0x0a, al

mov di, 0x8001
call parallel_read
mov al, [es:0x8001]
cmp al, 1                     ; exact supported cold/reset seconds
ja fail
mov [initial_second], al
mov si, fixed_date
mov di, 0x8002
mov cx, 4
repe cmpsb
jne fail

; Serial commands: 0011 capture, 0001 shift, both least significant bit first.
mov al, 3
call serial_command
mov al, 1
call serial_command
mov byte [control], 7
mov di, 0x8006
mov cx, 6
call read_bytes
mov bh, 4                     ; exactly 52 clocks, including command nibble
call read_bits
shr bl, 1
shr bl, 1
shr bl, 1
shr bl, 1
mov [es:di], bl
cmp bl, 1
jne fail
cmp byte [es:0x800b], 0x26
jne fail
mov si, 0x8001
mov di, 0x8006
push ds
push es
pop ds
mov cx, 5
repe cmpsb
pop ds
jne fail

mov bx, 0x800d
call bios_read
mov si, bios_date
mov di, 0x800d
mov cx, 5
repe cmpsb
jne fail
mov al, [initial_second]
cmp al, [es:0x8012]
jne fail

; Poll real ports until the calendar advances; IF remains clear throughout.
.wait:
mov di, 0x8013
call parallel_read
mov al, [es:0x8013]
cmp al, [initial_second]
je .wait
mov ah, [initial_second]
inc ah
cmp al, ah
jne fail
mov si, fixed_date
mov di, 0x8014
mov cx, 4
repe cmpsb
jne fail
mov bx, 0x8018
call bios_read
mov si, bios_date
mov di, 0x8018
mov cx, 5
repe cmpsb
jne fail
mov al, [es:0x8013]
cmp al, [es:0x801d]
jne fail
cmp byte [es:0x8000], 0xa5
jne fail
mov di, 0x801f
mov cx, 33
mov al, 0xa5
repe scasb
jne fail
mov byte [es:0x801e], 0x5a
mov si, pass_text
call report
hlt
jmp $

parallel_read:
mov al, 3
call command
mov al, 1
call command
mov byte [control], 1
mov cx, 5
read_bytes:
call read_byte
mov al, bl
stosb
loop read_bytes
ret
read_byte:
mov bh, 8
read_bits:
xor bl, bl
.bit:
in al, 0x33
mov ah, al
and ah, 0xfe
cmp ah, 0x38                  ; unrelated system-port bits must be preserved
jne fail
shr al, 1
rcr bl, 1
mov al, [control]
or al, 16
out 0x20, al
and al, 0xef
out 0x20, al                  ; output stable at trailing edge
; Read the initial LSB, then each next bit following CLK's trailing edge.
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
bios_read:
mov ax, 0x00ab
int 0x1c
cmp ax, 0x00ab
jne fail
ret
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
initial_second: db 0
fixed_date: db 0,0,1,0x14       ; minute, hour, day, month/weekday
bios_date: db 0x26,0x14,1,0,0   ; BIOS order, excluding variable second
pass_text: db 'RTC CALENDAR PASS',0
fail_text: db 'RTC CALENDAR FAIL',0
times 1024-($-$$) db 0
