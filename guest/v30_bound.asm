; Original V30 CHKIND/BOUND probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; nasm -f bin guest/v30_bound.asm -o v30_bound.bin
; One IPL sector at 1FC0:0000; guest-owned results/guards at 8000h..801Fh.
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
mov di, 0x8000
mov cx, 32
mov al, 0xa5
rep stosb
mov al, 0xff
out 2, al
out 0x0a, al
mov word [es:0x14], handler
mov ax, cs
mov [es:0x16], ax
mov byte [es:0x8001], 0
mov byte [es:0x8002], 0
mov word [es:0x8011], -5
mov word [es:0x8013], 10
; Inclusive signed lower, interior and upper cases preserve FLAGS and index.
mov ax, -5
push word 0xf5d7
popf
bound ax, [es:0x8011]
pushf
pop dx
cmp dx, 0xf5d7
jne fail
cmp ax, -5
jne fail
mov ax, 0
push word 0xf5d7
popf
bound ax, [es:0x8011]
pushf
pop dx
cmp dx, 0xf5d7
jne fail
cmp ax, 0
jne fail
mov ax, 10
push word 0xf5d7
popf
bound ax, [es:0x8011]
pushf
pop dx
cmp dx, 0xf5d7
jne fail
cmp ax, 10
jne fail
mov word [expected_ip], below_next
mov word [expected_flags], 0xf5d7 ; TF set, IF clear: delivery is unconditional
mov word [result_ptr], 0x8003
mov ax, -6
push word 0xf5d7
popf
jmp below

times 0x100-($-$$) db 0
below:
bound ax, [es:0x8011]
below_next:
pushf
pop dx
cmp dx, 0xf5d7
jne fail
cmp ax, -6
jne fail
inc byte [es:0x8002]
mov word [expected_ip], above_next
mov word [expected_flags], 0xf7d7 ; IF and TF set; both must clear on entry
mov word [result_ptr], 0x8009
mov ax, 11
push word 0xf7d7
popf
jmp above

times 0x140-($-$$) db 0
above:
bound ax, [es:0x8011]
above_next:
pushf
pop dx
cmp dx, 0xf7d7
jne fail
cmp ax, 11
jne fail
inc byte [es:0x8002]
cli
cld
cmp sp, 0x7c00
jne fail
mov byte [es:0x800f], 0x5a
; Every observed frame byte, bounds word, progress count and guard is checked.
mov si, expected
mov di, 0x8000
mov cx, 32
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

handler:
; Inspect entry flags before any arithmetic; leave the saved frame untouched.
pushf
pop dx
and dx, 0x0300
jnz fail
push bp
mov bp, sp
cmp bp, 0x7bf8
jne fail
mov dx, [ss:bp+2]
cmp dx, [expected_ip]
jne fail
mov di, [result_ptr]
mov [es:di], dx
mov dx, [ss:bp+4]
cmp dx, 0x1fc0
jne fail
mov [es:di+2], dx
mov dx, [ss:bp+6]
cmp dx, [expected_flags]
jne fail
mov [es:di+4], dx
inc byte [es:0x8001]
pop bp
iret

fail:
cli
cld
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
expected_ip: dw 0
expected_flags: dw 0
result_ptr: dw 0
pass_text: db 'V30 BOUND PASS', 0
fail_text: db 'V30 BOUND FAIL', 0
expected:
db 0xa5, 2, 2
; Exact next IP (including ES prefix), nonzero CS and original FLAGS twice.
dw 0x0105, 0x1fc0, 0xf5d7, 0x0145, 0x1fc0, 0xf7d7
db 0x5a, 0xa5
dw -5, 10
times 11 db 0xa5
times 1024-($-$$) db 0
