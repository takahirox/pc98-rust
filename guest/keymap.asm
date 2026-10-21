; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov dx,ready
mov ah,9
int 21h
xor ax,ax
mov es,ax
wait_down:
mov ax,0403h
int 18h
test ah,10h
jz wait_down
test byte [es:052dh],10h
jz stale
mov dx,downok
mov ah,9
int 21h
wait_up:
mov ax,0403h
int 18h
test ah,10h
jnz wait_up
test byte [es:052dh],10h
jnz stale
mov dx,ok
jmp print
stale: mov dx,bad
print:
mov ah,9
int 21h
mov ax,4c00h
int 21h
ready: db 'KEYMAP READY: hold Return then release',13,10,'$'
downok: db 'KEYMAP DOWN PASS',13,10,'$'
ok: db 'KEYMAP UP PASS',13,10,'$'
bad: db 'KEYMAP BWA STALE',13,10,'$'
