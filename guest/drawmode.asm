; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov ax,4a00h
mov cx,0600h
int 18h
mov dx,returned
mov ah,9
int 21h
mov ax,4c00h
int 21h
returned: db 'DRAWMODE RETURNED',13,10,'$'
