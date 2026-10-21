; Original Issue #23 drawing-timing control, SPDX-License-Identifier: MIT.
; No game/ROM/font content. Runs through guest FreeDOS and the real BIOS thunk.
bits 16
cpu 186
org 100h

; Every stage checks all 16-bit general/segment registers, SP and FLAGS.
; AL and CL are deliberately nonzero: only AH and CH select the service/mode.
%macro mode 3
mov ax,1234h
mov ds,ax
mov ax,2345h
mov es,ax
mov ax,4a5ah
mov cx,(%1 << 8) | 0abh
mov bx,1357h
mov dx,2468h
mov si,3579h
mov di,468ah
mov bp,579bh
mov [cs:saved_sp],sp
mov [cs:saved_ss],ss
push word 0647h ; IF, DF, ZF, PF, CF set, reserved bit 1
popf
int 18h
pushf
pop word [cs:returned_flags]
cmp ax,4a5ah
jne failed
cmp cx,(%1 << 8) | 0abh
jne failed
cmp bx,1357h
jne failed
cmp dx,2468h
jne failed
cmp si,3579h
jne failed
cmp di,468ah
jne failed
cmp bp,579bh
jne failed
cmp sp,[cs:saved_sp]
jne failed
mov ax,ss
cmp ax,[cs:saved_ss]
jne failed
mov ax,ds
cmp ax,1234h
jne failed
mov ax,es
cmp ax,2345h
jne failed
cmp word [cs:returned_flags],0647h
jne failed
cld
push cs
pop ds
xor ax,ax
mov es,ax
mov al,[es:054dh]
mov ah,al
and ah,8
cmp ah,%2
jne failed
mov bx,9000h
mov es,bx
mov [es:%3],al ; observed BIOS state for native/Wasm assertions
mov byte [es:0],%3
; Host tests inspect this stable stage, then inject Enter through the keyboard.
mov ah,0
int 18h
%endmacro

mode 06h,8,1
mode 16h,0,2
mode 16h,0,3 ; repeated selection
mode 06h,8,4 ; switch back, then reset tests clear the final flash request
mov dx,ok
mov ah,9
int 21h
mov ax,4c00h
int 21h

failed:
cld
push cs
pop ds
mov dx,bad
mov ah,9
int 21h
mov ax,4c01h
int 21h

saved_sp: dw 0
saved_ss: dw 0
returned_flags: dw 0
ok: db 'DRAW TIMING PASS',13,10,'$'
bad: db 'DRAW TIMING FAILED',13,10,'$'
