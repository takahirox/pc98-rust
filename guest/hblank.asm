; Original MIT DOS probe for Issue #15, based on the Issue #13 control.
; No game, ROM or driver content. DOS services execute inside guest FreeDOS.
bits 16
cpu 8086
org 100h
push cs
pop ds

; Initialize the supported 80x25 text and 640x400 color display via BIOS.
mov ax,0a01h
int 18h
mov ah,0ch
int 18h
mov ah,42h
mov ch,0c0h
int 18h
mov ah,40h
int 18h

mov dx,60h
call transitions
jc failed
mov dx,text_ok
mov ah,9
int 21h
mov dx,0a0h
call transitions
jc failed
mov dx,bitmap_ok
mov ah,9
int 21h
mov dx,ok
mov ah,9
int 21h
mov ax,4c00h
int 21h

failed:
mov dx,bad
mov ah,9
int 21h
mov ax,4c01h
int 21h

; Observe four set/clear pairs per port, each phase bounded by 65535 polls.
transitions:
mov bx,4
.next:
mov cx,0ffffh
.high:
in al,dx
test al,40h
jnz .seen_high
loop .high
jmp .timeout
.seen_high:
mov cx,0ffffh
.low:
in al,dx
test al,40h
jz .seen_low
loop .low
.timeout:
stc
ret
.seen_low:
dec bx
jnz .next
clc
ret

text_ok: db 'TEXT GDC 0060 PASS',13,10,'$'
bitmap_ok: db 'BITMAP GDC 00A0 PASS',13,10,'$'
bad: db 'HBLANK TRANSITION TIMEOUT',13,10,'$'
ok: db 'HBLANK TRANSITION PASS',13,10,'$'
