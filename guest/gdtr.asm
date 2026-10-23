; Original Ap real-mode GDTR boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/gdtr.asm -o gdtr.bin
; One 1024-byte sector loaded by the synthetic IPL at 1FC0:0000.
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
mov word [es:6*4], ud
mov word [es:13*4], gp
mov ax, cs
mov [es:6*4+2], ax
mov [es:13*4+2], ax
; Fixed, externally inspected result area, including every surrounding byte.
mov di, 0x8000
mov cx, 0x50
mov al, 0xa5
rep stosb
sgdt [es:0x8031]             ; deterministic emulator reset policy
cmp word [es:0x8031], 0
jne fail
cmp dword [es:0x8033], 0
jne fail
o32 lgdt [source]            ; nonzero high byte; base outside installed RAM
sgdt [es:0x8001]
o32 sgdt [es:0x8009]
mov di, 0x8001
call full
mov di, 0x8009
call full
lgdt [source]                ; default operand clears high base byte
sgdt [es:0x8011]
cmp word [es:0x8011], 0x1234
jne fail
cmp dword [es:0x8013], 0x00345678
jne fail
o32 lgdt [source]
sgdt [es:0xfffa]             ; last legal six-byte operand
cmp word [es:0xfffa], 0x1234
jne fail
cmp dword [es:0xfffc], 0x12345678
jne fail
mov byte [es:0xfff9], 0xa5
; Failed stores must leave all bytes unchanged, then retry at repaired BX.
mov word [expected_ip], store_fault
mov bx, 0xfffb
push ds
xor ax, ax
mov ds, ax
store_fault:
db 0x66, 0x3e, 0x0f, 0x01, 0x07 ; o32 sgdt [ds:bx]
pop ds
mov di, 0x8021
call full
cmp byte [es:0xfff9], 0xa5
jne fail
cmp word [es:0xfffa], 0x1234
jne fail
cmp dword [es:0xfffc], 0x12345678
jne fail
; SS-selected address32 outside 64K must be #GP (not #SS).
mov word [expected_ip], load_fault
mov si, source
mov di, 0x8029
mov cx, 6
rep movsb
mov ebp, 0x10000
load_fault:
db 0x66, 0x67, 0x0f, 0x01, 0x55, 0 ; o32 lgdt [ebp], default SS
sgdt [es:0x8039]
mov di, 0x8039
call full
; Register and LOCK encodings must fault at the first prefix.
mov word [expected_ip], reg_fault
mov word [resume_ip], reg_done
reg_fault:
db 0x66, 0x0f, 0x01, 0xd0
reg_done:
mov word [expected_ip], lock_fault
mov word [resume_ip], lock_done
lock_fault:
db 0x26, 0xf0, 0x0f, 0x01, 0x07
lock_done:
sgdt [es:0x8041]
mov di, 0x8041
call full
cmp byte [fault_count], 4
jne fail
mov al, [fault_count]
mov [es:0x8049], al
; Check all gaps, not just the first and last guard.
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
jmp report
full:
cmp word [es:di], 0x1234
jne fail
cmp dword [es:di+2], 0x12345678
jne fail
ret
; Bounded handlers verify exact vector, IP including prefixes, and CS.
; #GP repairs the address and returns to retry the SAME instruction.
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
inc byte [cs:fault_count]
cmp byte [cs:fault_count], 1
je .store
cmp byte [cs:fault_count], 2
jne fail
; Verify the failed load did not change GDTR, before retrying.
sgdt [es:0x8019]
mov di, 0x8019
call full
mov word [ss:bp], 0x8029
and ebp, 0xffff              ; repaired full EBP after POP BP
jmp .done
.store:
mov bx, 0x8021
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
mov ax, cs
cmp [ss:bp+4], ax
jne fail
inc byte [cs:fault_count]
cmp byte [cs:fault_count], 3
jb fail
cmp byte [cs:fault_count], 4
ja fail
mov ax, [cs:resume_ip]
mov [ss:bp+2], ax
pop ax
pop bp
iret
fail:
mov si, fail_text
report:
cli
push cs
pop ds
mov ax, 0xa000
mov es, ax
xor di, di
mov cx, 10                  ; bounded even if the terminator is damaged
.print:
lodsb
or al, al
jz .done
xor ah, ah
stosw
loop .print
.done:
hlt
jmp .done
source: dw 0x1234
        dd 0x12345678
expected_ip: dw 0
resume_ip: dw 0
fault_count: db 0
pass_text: db 'GDTR PASS', 0
fail_text: db 'GDTR FAIL', 0
guards: db 0,7,8,15,16,23,24,31,32,39,40,47,48,55,56,63,64,71,72,74,79
guards_end:
times 1024-($-$$) db 0
