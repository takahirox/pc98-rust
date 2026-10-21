; Original MIT Issue #16 probe. No game/ROM/font/driver data.
; BIOS font buffers start with scratch; patterns are left/right bytes per row.
bits 16
org 100h
start:
    push cs
    pop ds
    mov ax,9000h
    mov es,ax
    mov byte [es:0],0
    call define
    call read_and_draw
    mov byte [es:0],1
    xor ah,ah
    int 18h
    ; Replace the same code with a second asymmetric pattern.
    mov si,pattern
    mov cx,32
.invert:
    not byte [si]
    inc si
    loop .invert
    call define
    call read_and_draw
    mov byte [es:0],2
    xor ah,ah
    int 18h
    mov ax,4c00h
    int 21h

define:
    push ds
    push es
    push cs
    pop bx
    mov cx,buffer
    mov dx,7621h
    ; DS/ES deliberately differ from the font segment.
    mov ax,1234h
    mov ds,ax
    mov ax,2345h
    mov es,ax
    mov ax,1a69h
    int 18h
    pop es
    pop ds
    ret

read_and_draw:
    ; BIOS readback at 9000:0002; scratch remains at offsets 2..3.
    mov bx,9000h
    mov cx,2
    mov dx,7621h
    mov ah,14h
    int 18h
    mov si,pattern
    mov di,4
    mov cx,32
.check_bios:
    mov al,[si]
    cmp al,[es:di]
    jne fail
    inc si
    inc di
    loop .check_bios
    ; Select character: raw JIS cell, JIS row minus 20h.
    mov al,21h
    out 0a1h,al
    mov al,56h
    out 0a3h,al
    mov al,0bh
    out 68h,al
    mov ax,0a400h
    mov fs,ax
    mov ax,0a800h
    mov gs,ax
    xor bx,bx
    mov di,160*80+4 ; 16x16 blue glyph at (32,160)
.row:
    mov ax,bx
    or al,20h
    out 0a5h,al
    in al,0a9h
    ; CG ports and odd-address window must agree with BIOS and source.
    mov si,bx
    shl si,1
    cmp al,[pattern+si]
    jne fail
    mov [es:si+40h],al
    cmp al,[fs:si+1]
    jne fail
    mov [gs:di],al
    mov ax,bx
    out 0a5h,al
    in al,0a9h
    cmp al,[pattern+si+1]
    jne fail
    mov [es:si+41h],al
    cmp al,[fs:si+1]
    jne fail
    mov [gs:di+1],al
    inc bx
    add di,80
    cmp bx,16
    jb .row
    mov al,0ah
    out 68h,al
    ; Same glyph in text at (0,160), white; no host drawing.
    push es
    mov ax,0a000h
    mov es,ax
    mov word [es:1600],2156h
    mov word [es:1602],21d6h
    mov ax,0a200h
    mov es,ax
    mov byte [es:1600],0e1h
    mov byte [es:1602],0e1h
    pop es
    mov ah,40h
    int 18h
    ret
fail:
    mov byte [es:0],0ffh
    mov dx,failure
    mov ah,9
    int 21h
    mov ax,4c01h
    int 21h
failure: db 'USERFONT FAILED',13,10,'$'
buffer: dw 0a55ah ; scratch precedes pattern (NEC BIOS databook p.52)
pattern:
%assign y 0
%rep 16
    db (80h >> (y % 8)), (((y*2+1)*7) ^ 53h)
%assign y y+1
%endrep
