; Original 8237/FDC boot probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/dma_autoinit.asm -o autoinit.bin
; The synthetic IPL loads this one sector at 1FC0:0000. No DOS or guest ROM.
bits 16
cpu 186
org 0
cli
cld
xor ax, ax
mov ss, ax
mov sp, 0x7c00
mov es, ax
push cs
pop ds
; Own IR11 handler, with deterministic fresh PIC state (discard IPL IRQ).
mov word [es:0x4c], irq
mov ax, cs
mov [es:0x4e], ax
mov al, 0x11
out 0, al
out 8, al
mov al, 8
out 2, al
mov al, 0x10
out 0x0a, al
mov al, 0x80
out 2, al
mov al, 7
out 0x0a, al
mov al, 1
out 2, al
out 0x0a, al
mov al, 0x7f
out 2, al
mov al, 0xf7
out 0x0a, al
mov al, 0x18
out 0x94, al
mov byte [es:0x7fff], 0xa5
mov byte [es:0x8400], 0xa5
sti
mov al, 0x56
call program
mov byte [cmd+4], 2
mov byte [pattern], 0x39
call read_one
; Keep sticky DMA TC status through the second read; no DMA programming.
mov byte [cmd+4], 3
mov byte [pattern], 0xc7
call read_one
in al, 0x11
and al, 4
cmp al, 4
jne fail
in al, 0x11
and al, 4
jnz fail
mov al, 0x5a
call program
mov byte [cmd], 0x45
mov byte [cmd+4], 5
mov al, 0x69
call fill
call transfer
mov byte [cmd+4], 6
mov al, 0xb6
call fill
call transfer
in al, 0x11
and al, 4
cmp al, 4
jne fail
; Preserve the original final auto-init buffer assertion before reusing it.
mov al, 0xb6
mov di, 0x8000
mov cx, 1024
repe scasb
jne fail
; Focused software master-clear phase; retain all preceding auto-init checks.
mov word [failure], master_fail_text
mov byte [cmd], 0x46
mov byte [cmd+4], 2
mov al, 0x56
call program
mov al, 0xa5
call fill
call master_clear
call masked_transfer
; Masked read must leave every buffer byte untouched.
mov al, 0xa5
mov di, 0x8000
mov cx, 1024
repe scasb
jne fail
mov al, 2
out 0x15, al
mov byte [pattern], 0x39
call read_one
; Set up WRITE once; no reprogramming after master clear.
mov byte [cmd], 0x45
mov byte [cmd+4], 7
mov al, 0x5a
call program
mov al, 0x4d
call fill
call master_clear
call masked_transfer
; Sector 7 must remain unchanged (host checks the complete disk).
mov byte [cmd+4], 8
mov al, 2
out 0x15, al
call transfer
; Preserve the master-clear phase's final buffer check before reusing it.
mov al, 0x4d
mov di, 0x8000
mov cx, 1024
repe scasb
jne fail
; Controller-disable phase: keep channel 2 unmasked and retain programming.
mov word [failure], disable_fail_text
mov byte [cmd], 0x46
mov byte [cmd+4], 2
mov al, 0x56
call program
; Command writes must retain the high byte pointer and sticky TC.
in al, 9
mov al, 4
out 0x11, al
out 0x11, al
in al, 9
cmp al, 0x80
jne fail
in al, 0x11
cmp al, 4
jne fail
in al, 0x11
or al, al
jnz fail
mov al, 0xa5
call fill
call masked_transfer
mov al, 0xa5
mov di, 0x8000
mov cx, 1024
repe scasb
jne fail
in al, 0x11
or al, al
jnz fail
xor al, al
out 0x11, al
out 0x11, al
mov byte [pattern], 0x39
call read_one
; Disabled WRITE must not alter sector 7; host checks the complete image.
mov byte [cmd], 0x45
mov byte [cmd+4], 7
mov al, 0x5a
call program
mov al, 0xd2
call fill
mov al, 4
out 0x11, al
call masked_transfer
xor al, al
out 0x11, al
mov byte [cmd+4], 4
call transfer
mov si, pass_text
jmp report
master_clear:
; Start with the shared pointer high. The first subsequent access is readback.
in al, 9
mov al, 0x55
out 0x1b, al
call registers
in al, 0x11
or al, al
jnz fail
ret
masked_transfer:
mov byte [expected_st0], 0x40
mov byte [expected_st1], 0x10
call transfer
mov byte [expected_st0], 0
mov byte [expected_st1], 0
ret
read_one:
mov al, 0xa5
call fill
call transfer
mov al, [pattern]
mov di, 0x8000
mov cx, 1024
repe scasb
jne fail
ret
fill:
mov di, 0x8000
mov cx, 1024
rep stosb
ret
program:
out 0x17, al
xor al, al
out 0x19, al
out 9, al
mov al, 0x80
out 9, al
mov al, 0xff
out 0x0b, al
mov al, 3
out 0x0b, al
xor al, al
out 0x23, al
mov al, 2
out 0x15, al
ret
transfer:
mov bl, [irq_count]
inc bl
mov si, cmd
mov cx, 9
.send:
lodsb
out 0x92, al
loop .send
; Bounded waits for BOTH result availability and the actual completion IRQ.
mov cx, 0xffff
.wait_result:
in al, 0x90
and al, 0xc0
cmp al, 0xc0
je .result
loop .wait_result
jmp fail
.result:
mov cx, 0xffff
.wait_irq:
cmp [irq_count], bl
je .got_irq
loop .wait_irq
jmp fail
.got_irq:
in al, 0x92
cmp al, [expected_st0]
jne fail
in al, 0x92
cmp al, [expected_st1]
jne fail
in al, 0x92
or al, al
jnz fail
mov cx, 4
mov si, cmd+2
.chrn:
in al, 0x90
cmp al, 0xd0
jne fail
in al, 0x92
cmp al, [si]
jne fail
inc si
loop .chrn
in al, 0x90
cmp al, 0x80
jne fail
; Current address/count must already be reloaded, and guards untouched.
xor al, al
out 0x19, al
registers:
in al, 9
or al, al
jnz fail
in al, 9
cmp al, 0x80
jne fail
in al, 0x0b
cmp al, 0xff
jne fail
in al, 0x0b
cmp al, 3
jne fail
cmp byte [es:0x7fff], 0xa5
jne fail
cmp byte [es:0x8400], 0xa5
jne fail
ret
irq:
push ax
push ds
push cs
pop ds
inc byte [irq_count]
mov al, 0x20
out 8, al
out 0, al
pop ds
pop ax
iret
fail:
mov si, [failure]
report:
cli
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
cmd: db 0x46, 0, 0, 0, 2, 3, 8, 0x1b, 0xff
pattern: db 0
irq_count: db 0
failure: dw fail_text
expected_st0: db 0
expected_st1: db 0
pass_text: db 'DMA AUTO INIT PASS / DMA MASTER CLEAR PASS / DMA DISABLE PASS', 0
disable_fail_text: db 'DMA DISABLE FAIL', 0
master_fail_text: db 'DMA MASTER CLEAR FAIL', 0
fail_text: db 'DMA AUTO INIT FAIL', 0
times 1024-($-$$) db 0
