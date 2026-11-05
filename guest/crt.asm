; Original rights-cleared PC-98 CRT/KCG probe. SPDX-License-Identifier: MIT
; Copyright (c) 2026 pc98-rust contributors. See ../LICENSE.
; No DOS, ROM dump, private font or game data. BIOS contracts: NEC 1992 pp.41-55.
; 1Ch..1Eh exercise the explicitly documented Ap compatibility extension.
bits 16
cpu 186
org 0
%ifndef FIRST
%define FIRST 0x1b
%endif
cli
xor ax,ax
mov ss,ax
mov sp,0x7c00
push cs
pop es
mov bp,1024
mov bx,3072
mov cx,0x0300
mov dx,2
mov ax,0x5690
int 0x1b
jc $
jmp start
times 1024-($-$$) db 0
start:
cld
push cs
pop ds
xor ax,ax
mov es,ax
mov di,0x8000
mov cx,128
mov al,0xa5
rep stosb
mov byte [es:0x8001],0
mov al,0xff
out 2,al
out 0x0a,al
; Compile-time first-service variants independently reproduce each old trap.
%if FIRST = 0x0f
push cs
pop bx
mov cx,areas
mov dx,2
mov ax,0x0f00
%elif FIRST = 0x1c
mov ax,0x1c40
mov dx,0x0e0f
%elif FIRST = 0x1d
mov ax,0x1d64
%elif FIRST = 0x1e
mov ax,0x1e00
mov dx,0x0e0f
%else
mov ax,0x1b00
%endif
call bios
; Combined sequence: initialize, dot/code, area and width, cursor, restore.
mov ax,0x1c40
mov dx,0x0e0f
call bios
mov ax,0x0c00
call bios
mov ax,0x0b69
call bios
cmp al,0
jne fail
cmp byte [es:0x53c],0
jne fail
; BIOS initialization must clear all text RAM, including off-screen word 3000.
mov ax,0xa000
mov fs,ax
mov word [fs:6000],0x115a
mov ax,0xa200
mov fs,ax
mov word [fs:6000],0x1111
mov ax,0x1600
mov dx,0xe120
call bios
mov ax,0xa000
mov fs,ax
cmp word [fs:6000],0x0020
jne fail
cmp word [fs:8190],0x0020
jne fail
mov ax,0xa200
mov fs,ax
cmp word [fs:6000],0x00e1
jne fail
cmp word [fs:8190],0x00e1
jne fail
push cs
pop bx
mov cx,glyph_buffer
mov dx,0x7621
mov ax,0x1a69
call bios
mov bx,0
mov cx,0x8020
mov ax,0x1469
call bios
mov si,glyph
mov di,0x8022
mov cx,32
repe cmpsb
jne fail
cmp word [es:0x8020],0xa5a5
jne fail
cmp word [es:0x8042],0xa5a5
jne fail
; Both ROM and user RAM reads use real CG port/window instructions.
mov ax,0x1b01
call bios
cmp byte [es:0x53c],8
jne fail
mov al,0x21
out 0xa1,al
mov al,0x56
out 0xa3,al
mov ax,0xa400
mov fs,ax
xor bx,bx
.read_cg:
mov ax,bx
or al,0x20
out 0xa5,al
in al,0xa9
mov si,bx
shl si,1
cmp al,[glyph+si]
jne fail
cmp al,[fs:si+1]
jne fail
mov [es:si+0x8044],al
mov ax,bx
out 0xa5,al
in al,0xa9
cmp al,[glyph+si+1]
jne fail
cmp al,[fs:si+1]
jne fail
mov [es:si+0x8045],al
inc bx
cmp bx,16
jb .read_cg
mov al,0x22
out 0xa1,al
mov al,4
out 0xa3,al
mov al,0x20
out 0xa5,al
in al,0xa9
cmp al,[fs:0]
jne fail
mov [es:0x8004],al
mov ax,0x1b00
call bios
cmp byte [fs:1],0xff
jne fail
; ANK BX:CX destination, two untouched scratch bytes and 8 rows.
mov bx,0
mov cx,0x8008
mov dx,0x0041
mov ax,0x1400
call bios
cmp word [es:0x8008],0xa5a5
jne fail
cmp byte [es:0x800a],0x38
jne fail
cmp word [es:0x8012],0xa5a5
jne fail
mov ax,0x1d64                  ; physical pitch 100 words, visible 80
call bios
push cs
pop bx
mov cx,areas
mov dx,2
mov ax,0x0f69
call bios
; Update partition 1 alone; partition 0 must survive.
mov cx,area_update
mov dx,0x0101
call bios
; Two areas: first row starts 0, remaining rows start 500 with pitch 100.
push es
mov ax,0xa000
mov es,ax
mov word [es:0],0x2156            ; registered left and right halves
mov word [es:2],0x21d6
mov word [es:1000],0x0041         ; A at (0,16)
mov word [es:1200],0x0042         ; B at (0,32), distinguishes pitch from 80
mov word [es:200],0x0043          ; decoy if partition selection is ignored
pop es
mov ax,0x1300
mov dx,10                     ; BIOS cursor in blank cell 5, then direct I/O to 4
call bios
mov al,0x49
out 0x62,al
mov al,4
out 0x60,al
xor al,al
out 0x60,al                   ; legal two-byte character-mode CURS
mov al,0xe0
out 0x62,al
in al,0x62
cmp al,4
jne fail
in al,0x62
or al,al
jne fail
in al,0x62
or al,al
jne fail
in al,0x62
in al,0x62
mov ax,0x1e00
mov dx,0x0e0f
call bios
mov al,0xfd
out 2,al
mov byte [es:0x8001],1
call wait_key
; Direct I/O dot access blanks only Kanji; cursor and ANK remain visible.
mov al,0x0b
out 0x68,al
mov byte [es:0x8001],2
call wait_key
; BIOS switches back and enables blink: on/off each 4 guest video frames.
mov ax,0x1b00
call bios
mov ax,0x1e82
mov dx,0x0e0f
call bios
mov byte [es:0x8001],3
call wait_key
; 40 columns, 20 rows: set and sense encoding 3.
mov ax,0x0a03
call bios
mov ax,0x0b69
call bios
cmp al,3
jne fail
cmp byte [es:0x53c],3
jne fail
mov ax,0x1001
call bios
mov ax,0x1200
call bios
mov ax,0x0e00
mov dx,0
call bios
mov byte [es:0x8001],4
call wait_key
; Restore the normal single-area, pitch, code access and hidden cursor.
mov ax,0x0a00
call bios
mov ax,0x1d50
call bios
mov ax,0x1200
call bios
mov ax,0x0d00
call bios
mov ax,0x0c00
call bios
mov byte [es:0x8001],0x5a
mov si,pass_text
call report
cli
hlt
jmp $

; Compare every documented preserved register, segment and flags through the
; actual INT -> executable ROM OUT -> IRET path. Only 0Bh returns AL.
bios:
pushf
pusha
push ds
push es
int 0x18
pushf
pusha
push ds
push es
mov bp,sp
; For sense, compare saved AH but AL is the one documented result.
cmp byte [ss:bp+19],0x0b
jne .compare
mov al,[ss:bp+18]
mov [ss:bp+40],al
.compare:
xor si,si
.loop:
cmp si,10                      ; PUSHA saved SP differs by the frame size
je .next
mov ax,[ss:bp+si]
cmp ax,[ss:bp+si+22]
jne fail
.next:
add si,2
cmp si,22
jb .loop
pop es
pop ds
popa
popf
add sp,22                      ; discard the older snapshot
ret
wait_key:
xor ah,ah
int 0x18
ret
report:
push es
mov ax,0xa000
mov es,ax
xor di,di
.print:
lodsb
or al,al
jz .done
xor ah,ah
stosw
jmp .print
.done:
pop es
ret
fail:
mov byte [es:0x8001],0xff
mov si,fail_text
call report
cli
hlt
jmp $
areas: dw 0,1,400,24
area_update: dw 500,24
pass_text: db 'CRT FAMILY PASS',0
fail_text: db 'CRT FAMILY FAIL',0
glyph_buffer: dw 0x5aa5
glyph:
%assign y 0
%rep 16
 db (0x80 >> (y % 8)), (((y*2+1)*7) ^ 0x53)
%assign y y+1
%endrep
times 4096-($-$$) db 0
