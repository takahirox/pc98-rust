; Original Ap real-mode IDTR boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/idtr.asm -o idtr.bin
; Synthetic IPL loads this one-sector probe at 1FC0:0000.
bits 16
cpu 486
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
mov cx, 80
mov al, 0xa5
rep stosb
sidt [es:0x8001]
cmp word [es:0x8001], 0x3ff
jne fail
cmp dword [es:0x8003], 0
jne fail
; An unmapped high base is metadata, not a table access at load time.
o32 lidt [high]
sidt [es:0x8009]
o32 sidt [es:0x8011]
a32 sidt [es:dword 0x8031]
cmp dword [es:0x800b], 0x12345678
jne fail
cmp dword [es:0x8013], 0x12345678
jne fail
cmp dword [es:0x8033], 0x12345678
jne fail
lidt [high]
sidt [es:0x8019]
cmp dword [es:0x801b], 0x00345678
jne fail
; Unaligned relocated table; fixed IVT points to FAIL for every used vector.
mov di, 0x6001
mov cx, 256
.table:
mov ax, event
stosw
mov ax, cs
stosw
loop .table
mov word [es:0x6001+8*4], double_fault
mov word [es:0x6001+13*4], gp
mov word [es:0x6001+6*4], ud
mov di, 0
mov cx, 256
.fixed:
mov ax, fail
stosw
mov ax, cs
stosw
loop .fixed
lidt [table]
sidt [es:0x8021]
sidt [es:0xfffa]
mov byte [es:0xfff9], 0xa5
mov byte [es:0x8029], 0
mov byte [es:0x802a], 0
mov byte [es:0x802b], 0
; A complete last entry (vector 30h) fits inclusive limit C3h.
mov word [expected_ip], int_done
mov word [expected_flags], 2
push word 2
popf
int 0x30
int_done:
mov word [expected_ip], int3_done
int3
int3_done:
mov word [expected_ip], into_done
push word 0x802
popf
mov word [expected_flags], 0x802
into
into_done:
push word 2
popf
into                            ; untaken; count must stay three
cmp byte [es:0x8029], 3
jne fail
; Faulting SIDT must not write any of the FFFA result; repair and retry.
mov word [expected_ip], store_fault
mov bx, 0xfffb
store_fault:
db 0x66, 0x26, 0x0f, 0x01, 0x0f ; o32 sidt [es:bx]
cmp word [es:0xfffa], 0xc3
jne fail
cmp dword [es:0xfffc], 0x6001
jne fail
cmp byte [es:0xfff9], 0xa5
jne fail
; SS-selected address32 operand gets #GP, and retries the first prefix.
mov ebp, 0x10000
mov word [expected_ip], load_fault
load_fault:
db 0x66, 0x67, 0x0f, 0x01, 0x5d, 0 ; o32 lidt [ss:ebp]
; Invalid register and LOCK forms, with bounded #UD skip handlers.
mov word [expected_ip], reg_fault
mov word [resume_ip], reg_done
reg_fault:
db 0x66, 0x0f, 0x01, 0xd8
reg_done:
mov word [expected_ip], lock_fault
mov word [resume_ip], lock_done
lock_fault:
db 0x26, 0xf0, 0x0f, 0x01, 0x0f
lock_done:
cmp byte [es:0x802a], 2
jne fail
cmp byte [es:0x802b], 2
jne fail
; Force 32-bit base+vector wrap. Address zero holds vector 1, while
; fixed-IVT vector 1 (address 4) still points to FAIL. Same on native/Wasm.
mov word [es:0], wrap_event
mov ax, cs
mov [es:2], ax
o32 lidt [wrap_table]
int 1
cmp byte [es:0x8041], 0x77
jne fail
lidt [short_table]               ; 23h permits #DF, excludes INT 30h
mov word [expected_ip], limit_int
push word 2
popf
limit_int:
db 0x66, 0x2e, 0xcd, 0x30
limit_done:
cmp byte [es:0x8039], 0x88
jne fail
; Inspect every gap and guard before reporting.
mov si, guards
mov cx, guards_end-guards
.guard:
lodsb
xor ah, ah
mov di, ax
cmp byte [es:di+0x8000], 0xa5
jne fail
loop .guard
mov si, pass_text
call report
; Terminal case: #DF's own entry is one byte short. No following write,
; HLT, recursive loop or host error is allowed; host checks shutdown directly.
lidt [shutdown_table]
int 0x30
mov byte [es:0x8042], 0xee
jmp fail

event:
push bp
mov bp, sp
push ax
mov ax, [cs:expected_ip]
cmp [ss:bp+2], ax
jne fail
mov ax, cs
cmp [ss:bp+4], ax
jne fail
mov ax, [cs:expected_flags]
cmp [ss:bp+6], ax
jne fail
inc byte [es:0x8029]
pop ax
pop bp
iret
wrap_event:
mov byte [es:0x8041], 0x77
iret
double_fault:
; No error code: saved IP is immediately at SP, FLAGS exactly six bytes up.
push bp
mov bp, sp
push ax
mov ax, [cs:expected_ip]
cmp [ss:bp+2], ax
jne fail
mov ax, cs
cmp [ss:bp+4], ax
jne fail
cmp word [ss:bp+6], 2
jne fail
mov word [ss:bp+2], limit_done
mov byte [es:0x8039], 0x88
pop ax
pop bp
iret
gp:
push bp
mov bp, sp
push ax
mov ax, [cs:expected_ip]
cmp [ss:bp+2], ax
jne fail
mov ax, cs
cmp [ss:bp+4], ax
jne fail
inc byte [es:0x802a]
cmp byte [es:0x802a], 1
je .store
cmp byte [es:0x802a], 2
jne fail
mov word [ss:bp], 0x8021       ; repair EBP to saved valid descriptor
and ebp, 0xffff
jmp .done
.store:
mov bx, 0x8049
.done:
pop ax
pop bp
iret
ud:
push bp
mov bp, sp
push ax
mov ax, [cs:expected_ip]
cmp [ss:bp+2], ax
jne fail
inc byte [es:0x802b]
cmp byte [es:0x802b], 2
ja fail
mov ax, [cs:resume_ip]
mov [ss:bp+2], ax
pop ax
pop bp
iret
fail:
mov si, fail_text
call report
cli
hlt
jmp fail
report:
push es
mov ax, 0xa000
mov es, ax
xor di, di
mov cx, 10
.print:
lodsb
or al, al
jz .done
xor ah, ah
stosw
loop .print
.done:
pop es
ret
high: dw 0x3ff
      dd 0x12345678
table: dw 0xc3
       dd 0x6001
short_table: dw 0x23
             dd 0x6001
shutdown_table: dw 0x22
                dd 0x6001
wrap_table: dw 7
            dd 0xfffffffc
expected_ip: dw 0
resume_ip: dw 0
expected_flags: dw 2
pass_text: db 'IDTR PASS', 0
fail_text: db 'IDTR FAIL', 0
guards: db 0,7,8,15,16,23,24,31,32,39,40,44,47,48,55,56,58,63,64,66,71,72,79
guards_end:
times 1024-($-$$) db 0
