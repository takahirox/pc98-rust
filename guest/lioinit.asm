; Original Issue #21 research probe, SPDX-License-Identifier: MIT.
bits 16
org 100h
push cs
pop ds
mov ax,35a0h
int 21h
mov [oldoff],bx
mov [oldseg],es
mov ax,0f990h
mov ds,ax
mov dx,[6] ; GINIT offset in the ROM entry table
mov ax,25a0h
int 21h
push cs
pop ds
mov dx,entering
mov ah,9
int 21h
mov ax,cs
add ax,(work-$$+100h)/16
mov ds,ax
int 0a0h
push cs
pop ds
mov [result],ah
mov dx,returned
mov ah,9
int 21h
mov dx,[oldoff]
mov ax,[oldseg]
mov ds,ax
mov ax,25a0h
int 21h
mov ah,4ch
mov al,[cs:result]
int 21h
oldoff: dw 0
oldseg: dw 0
result: db 0
entering: db 'LIO ENTER',13,10,'$'
returned: db 'LIO RETURNED',13,10,'$'
align 16
work: times 1200h db 0
