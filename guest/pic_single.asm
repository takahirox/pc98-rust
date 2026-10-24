; Original SNGL boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/pic_single.asm -o pic_single.bin
; Synthetic IPL loads one 1024-byte sector at 1FC0:0000.
bits 16
cpu 8086
org 0
%macro snapshot 3
mov al, %2
out %1, al
in al, %1
mov [es:0x8000+%3], al
%endmacro
%macro vector 2
mov word [es:%1*4], %2
mov ax, cs
mov [es:%1*4+2], ax
%endmacro
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
xor al, al
mov [es:0x8001], al
mov [es:0x8006], al
mov [es:0x800b], al
mov [es:0x8021], al
vector 0x20, normal_timer
vector 0x28, auto_timer
vector 0x27, local_fdc
vector 0x53, cascade_fdc
; Slave normal EOI, FDC initially masked. Master single, normal EOI.
mov al, 0x11
out 8, al
mov al, 0x50
out 10, al
mov al, 7
out 10, al
mov al, 1
out 10, al
mov al, 0xff
out 10, al
mov al, 0x13
out 0, al
mov al, 0x27                  ; deliberately unaligned ICW2 -> vector 20h
out 2, al
mov al, 1
out 2, al
mov al, 0xfe
out 2, al
call start_timer
sti
nop
.wait_normal:
cmp byte [es:0x8001], 1
jb .wait_normal
cli
snapshot 0, 0x0b, 5
; Reinitialize single mode with AEOI; next data byte is already OCW1.
mov al, 0x13
out 0, al
mov al, 0x2b
out 2, al
mov al, 3
out 2, al
mov al, 0xfe
out 2, al
call start_timer
sti
nop
.wait_auto:
cmp byte [es:0x8006], 1
jb .wait_auto
cli
snapshot 0, 0x0b, 10
; Single-mode IR7 comes from the physical slave INT wire, but is local.
mov al, 0x13
out 0, al
mov al, 0x20
out 2, al
mov al, 1
out 2, al
mov al, 0x7f
out 2, al
mov al, 0xf7
out 10, al
call read_id
sti
nop
.wait_local:
cmp byte [es:0x800b], 1
jb .wait_local
cli
snapshot 0, 0x0b, 18
; Keep slave INT asserted through EOI, IRET, time and PIC port operations.
; Enabling IF here catches any duplicate local IR7 entries as count != 1.
sti
nop
mov cx, 1000
.held:
in al, 0
in al, 10
loop .held
cli
mov al, [es:0x800b]
mov [es:0x8013], al
snapshot 0, 0x0a, 20
snapshot 8, 0x0a, 21
; Separately drain the slave through its guest poll, then the FDC result.
snapshot 8, 0x0c, 22
snapshot 8, 0x0b, 23
snapshot 0, 0x0b, 24
mov di, 0x801a
call drain_id
mov al, 0x20
out 8, al
snapshot 8, 0x0b, 25
; Fresh quiescent inputs: return to fixed PC-98 cascading, normal EOI.
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x50
out 10, al
mov al, 0x80
out 2, al
mov al, 7
out 10, al
mov al, 1
out 2, al
out 10, al
mov al, 0x7f
out 2, al
mov al, 0xf7
out 10, al
call read_id
sti
nop
.wait_cascade:
cmp byte [es:0x8021], 1
jb .wait_cascade
cli
snapshot 0, 0x0b, 40
mov di, 0x8029
call drain_id
; Independent expected bytes, including guards, checked by guest and hosts.
mov si, expected
mov di, 0x8000
mov cx, 64
.check:
lodsb
cmp al, [es:di]
jne fail
inc di
loop .check
mov si, pass_text
call report
hlt
jmp $
start_timer:
mov al, 0x30                  ; mode 0 one-shot, divisor 1000
out 0x77, al
mov al, 0xe8
out 0x71, al
mov al, 3
out 0x71, al
ret
read_id:
mov al, 0x4a                  ; FDC READ ID, drive 0/head 0; no writes
out 0x92, al
xor al, al
out 0x92, al
ret
drain_id:
mov cx, 7
.next:
in al, 0x92
stosb
loop .next
ret
normal_timer:
push ax
inc byte [es:0x8001]
snapshot 0, 0x0b, 2
snapshot 0, 0x0a, 3
in al, 2
mov [es:0x8004], al
mov al, 0x20
out 0, al
pop ax
iret
auto_timer:
push ax
inc byte [es:0x8006]
snapshot 0, 0x0b, 7
snapshot 0, 0x0a, 8
in al, 2
mov [es:0x8009], al
; No EOI: AEOI must leave ISR empty.
pop ax
iret
local_fdc:
push ax
inc byte [es:0x800b]
snapshot 0, 0x0b, 12
snapshot 0, 0x0a, 13
snapshot 8, 0x0b, 14
snapshot 8, 0x0a, 15
in al, 2
mov [es:0x8010], al
in al, 10
mov [es:0x8011], al
mov al, 0x20
out 0, al                     ; master EOI only, slave remains pending
pop ax
iret
cascade_fdc:
push ax
inc byte [es:0x8021]
snapshot 0, 0x0b, 34
snapshot 8, 0x0b, 35
snapshot 0, 0x0a, 36
snapshot 8, 0x0a, 37
mov al, 0x20
out 8, al
snapshot 0, 0x0b, 38           ; slave EOI leaves master ISR7
snapshot 8, 0x0b, 39
mov al, 0x20
out 0, al
pop ax
iret
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
pass_text: db 'PIC SINGLE PASS', 0
fail_text: db 'PIC SINGLE FAIL', 0
expected:
; Normal PIT count/ISR/IRR/IMR/post-EOI; AEOI PIT equivalents.
db 0xa5,1,1,0,0xfe,0,1,0,0,0xfe,0
; Local IR7 count, master ISR/IRR, slave ISR/IRR, masks, master EOI.
db 1,0x80,0,0,8,0x7f,0xf7,0
; Held INT count/master IRR/slave IRR; slave poll/ISR/master ISR/slave EOI.
db 1,0,8,0x83,8,0,0
; READ ID ST0/ST1/ST2/C/H/R/N.
db 0,0,0,0,0,1,3
; Cascade count, both ISRs, both IRRs, slave EOI snapshots, master EOI.
db 1,0x80,8,0,0,0x80,0,0
; Second READ ID and untouched trailing guards.
db 0,0,0,0,0,1,3
times 16 db 0xa5
times 1024-($-$$) db 0
