; Original paging-disabled PC-98 protected-mode acceptance guest.
; SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; One IPL sector at 1FC0:0000. No DOS, ROM dump, or external CPU code.
bits 16
cpu 486
org 0
%define BOOT 0x1fc00
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
; Establish a precise initial frame/IRQ result area, leaving all other guards.
mov dword [es:0x8001], 0
mov dword [es:0x8009], 0
mov byte [es:0x8011], 0
mov byte [es:0x8019], 0
; PC-98 A20 latch (not IBM port 92h).
mov al, 2
out 0xf2, al
lgdt [gdt_ptr]
lidt [idt_ptr]
; Construct IDT in RAM; only selected guest-owned entries are present.
%macro gate 4
mov word [es:0x9000+8*%1], (BOOT+(%2-$$)) & 0xffff
mov word [es:0x9002+8*%1], %3
mov word [es:0x9004+8*%1], %4 << 8
mov word [es:0x9006+8*%1], (BOOT+(%2-$$)) >> 16
%endmacro
gate 0x30, software, 0x10, 0x8f ; 386 trap gate preserves IF
gate 13, gp, 0x10, 0x8e       ; 386 interrupt gate, error code
gate 8, timer, 0x10, 0x8e     ; actual master PIC IRQ0
mov eax, cr0
or al, 1
pe_write: mov cr0, eax
; Cached real-mode CS remains in use until this far JMP.
jmp 0x08:pm16
pm16:
mov ax, 0x18
mov ds, ax
mov es, ax
mov fs, ax
mov ax, 0x38
lldt ax
mov ax, 4                     ; first LDT entry, flat writable data
mov gs, ax
mov ax, 0x20
mov ss, ax
mov esp, 0x19000               ; SS.B=1, EIP/ESP above 64 KiB
mov byte [0x8001], 0x16
jmp dword 0x10:BOOT+pm32
bits 32
pm32:
smsw ax
test ax, 1
jz fail32
mov eax, cr0
test eax, 1
jz fail32
mov dword [0x8009], 0x32323232
mov byte [0x210000], 0xa5
mov byte [0x210005], 0xa5
mov byte [0x18fe0], 0xa5
mov byte [0x19000], 0xa5
mov dword [gs:0x210001], 0x1234abcd
cmp dword [fs:0x210001], 0x1234abcd
jne fail32
mov bx, 0x8011
mov byte [bx], 0x67             ; 67 address override in D=1 code
mov word [0x8019], 0x6655       ; 66 operand override
cmp word [0x8019], 0x6655
jne fail32
push dword 0x13579bdf
pop edx
cmp edx, 0x13579bdf
jne fail32
call near_probe
call dword 0x10:BOOT+far_probe
int 0x30
bad_load:
mov ax, 0x40                   ; beyond GDT limit: recoverable #GP(40h)
mov ds, ax
cmp byte [0x8003], 1
jne fail32
; PIC ICWs and PIT one-shot are guest I/O, no host IRQ injection.
mov al, 0xff
out 0x0a, al
mov al, 0x30
out 0x77, al
mov al, 0x11
out 0, al
mov al, 8
out 2, al
mov al, 0x80
out 2, al
mov al, 1
out 2, al
mov al, 0xfe
out 2, al
mov al, 0x30
out 0x77, al
mov ax, 1000
out 0x71, al
mov al, ah
out 0x71, al
; IF=0: wait long enough for PIT terminal; IRQ must remain pending.
mov ecx, 2000
.quiet: loop .quiet
cmp byte [0x8004], 0
jne fail32
sti
nop
mov ecx, 6000
.wait:
cmp byte [0x8004], 1
je .received
loop .wait
jmp fail32
.received:
cli
mov al, 0xff
out 2, al
cmp esp, 0x19000
jne fail32
jmp word 0x08:pm16_exit
near_probe:
mov byte [0x801a], 0x4e
ret
far_probe:
mov byte [0x801b], 0x46
retf
software:
cmp esp, 0x18ff4
jne fail32
pushfd
pop eax
test eax, 0x100                 ; TF cleared on entry
jnz fail32
inc byte [0x8002]
iretd
gp:
cmp esp, 0x18ff0
jne fail32
cmp dword [ss:esp], 0x40
jne fail32
cmp dword [ss:esp+4], BOOT+bad_load+4 ; MOV DS starts after MOV AX,40h
jne fail32
mov word [ss:esp+4], (BOOT+bad_load-$$+6) & 0xffff ; skip rejected MOV DS
inc byte [0x8003]
add esp, 4                     ; discard architectural error code
iretd
timer:
push eax
mov al, 0x0b
out 0, al
in al, 0
cmp al, 1                      ; real PIC ISR before EOI
jne fail32
inc byte [0x8004]
mov al, 0x20
out 0, al
mov al, 0x0b
out 0, al
in al, 0
cmp al, 0
jne fail32
pop eax
iretd
fail32:
cli
mov dword [0xa0000], 0x00500050 ; visible 'PP' diagnostic; never PASS
hlt
jmp $
bits 16
pm16_exit:
mov ax, 0x28                   ; real-compatible, writable 64 KiB data/stack
mov ss, ax
mov sp, 0x7c00
mov ds, ax
mov es, ax
mov fs, ax
mov gs, ax
mov eax, cr0
and al, 0xfe
mov cr0, eax
jmp 0x1fc0:real_again
real_again:
push cs
pop ds
lidt [real_idt]
mov eax, cr0
test al, 1
jnz fail
cmp byte [es:0x8001], 0x16
jne fail
cmp word [es:0x8002], 0x0101
jne fail
cmp byte [es:0x8004], 1
jne fail
cmp dword [es:0x8009], 0x32323232
jne fail
mov byte [es:0x8021], 0x52
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
jmp report
fail:
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
cli
hlt
jmp $
align 8
; limit low, base low, base mid, access, limit-hi/G/D, base hi
gdt:
dq 0
dw 0xffff, BOOT & 0xffff
db BOOT >> 16, 0x9a, 0, 0      ; 08: code16 base BOOT
dw 0xffff, 0
db 0, 0x9a, 0xcf, 0            ; 10: flat code32
dw 0xffff, 0
db 0, 0x92, 0xcf, 0            ; 18: flat data32
dw 0xffff, 0
db 0, 0x92, 0x43, 0            ; 20: stack32, limit 3ffffh
dw 0xffff, 0
db 0, 0x92, 0, 0               ; 28: data16/stack16
dq 0                           ; 30: reserved/null descriptor
dw 7, (BOOT+ldt-$$) & 0xffff
db (BOOT+ldt-$$) >> 16, 0x82, 0, 0 ; 38: LDT
.end:
ldt:
dw 0xffff, 0
db 0, 0x92, 0xcf, 0
; Tables are preserved across mode transitions; guest restores real IDTR.
gdt_ptr: dw gdt.end-gdt-1
 dd BOOT+gdt
idt_ptr: dw 0x187
 dd 0x9000
real_idt: dw 0x3ff
 dd 0
expected:
db 0xa5,0x16,1,1,1
 times 4 db 0xa5
 dd 0x32323232
 times 4 db 0xa5
 db 0x67
 times 7 db 0xa5
 db 0x55,0x4e,0x46
 times 5 db 0xa5
 db 0x52
 times 30 db 0xa5
pass_text: db 'PROTECTED MODE PASS',0
fail_text: db 'PROTECTED MODE FAIL',0
times 1024-($-$$) db 0
