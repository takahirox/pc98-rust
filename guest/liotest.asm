; Original GINIT contract control. SPDX-License-Identifier: MIT.
bits 16
cpu 8086
org 100h
push cs
pop ds
cld
; Save and register an alternate DOS vector to prove dispatch uses ROM entry,
; rather than special-casing INT A0. Check the entire published directory.
mov ax,3566h
int 21h
mov [oldoff],bx
mov [oldseg],es
mov ax,0f990h
mov es,ax
cmp byte [es:0],11h
jne failed
mov si,4
mov dl,0a0h
mov cx,16
.directory:
cmp [es:si],dl
jne failed
cmp byte [es:si+1],0
jne failed
cmp word [es:si+2],0
je failed
inc dl
add si,4
loop .directory
cmp word [es:si],0ceh
jne failed
cmp word [es:si+4],0
jne failed
mov dx,[es:6]
mov ax,es
mov ds,ax
mov ax,2566h
int 21h
push cs
pop ds
mov [savedss],ss
mov [savedsp],sp
; Work lives inside this DOS allocation, with guards and the documented gaps.
mov ax,cs
add ax,(work-$$+100h)/16
mov [workseg],ax
mov bp,2
.repeat:
mov ax,[cs:workseg]
mov es,ax
mov di,0
mov cx,1200h
mov al,0a5h
rep stosb
; Dirty bank selection, palette and display through real guest I/O.
mov al,1
out 0a4h,al
out 0a6h,al
mov al,0
out 0a8h,al
out 0aah,al
out 0ach,al
out 0aeh,al
mov al,0ch
out 0a2h,al
; Pixel controls away from shell text: first logical screen blue/red rows;
; the hidden second half is green. GINIT initializes the view, not GCLS.
mov al,0
out 0a6h,al
mov ax,0a800h
mov es,ax
mov byte [es:150*80+79],80h
mov byte [es:151*80+79],0
mov byte [es:16000+150*80+79],0
mov ax,0b000h
mov es,ax
mov byte [es:150*80+79],0
mov byte [es:151*80+79],80h
mov ax,0b800h
mov es,ax
mov byte [es:150*80+79],0
mov byte [es:151*80+79],0
mov byte [es:16000+150*80+79],80h
mov al,1
out 0a6h,al
mov ds,[cs:workseg]
push bp
std
mov ah,0ffh
int 66h
cld
cmp ah,0
jne failed
mov ax,ds
cmp ax,[cs:workseg]
jne failed
mov ax,ss
cmp ax,[cs:savedss]
jne failed
pop bp
cmp sp,[cs:savedsp]
jne failed
push cs
pop ds
cmp word [before],05aa5h
jne failed
cmp word [after],0a55ah
jne failed
; The public contract leaves the common area opaque. Check only its bounds,
; that it changed, and that the caller-owned gaps retain their sentinel.
mov ax,[workseg]
mov es,ax
mov di,0
mov cx,620h
call gap
mov di,6a0h
mov cx,960h
call gap
cmp byte [es:620h],0a5h
je failed
mov di,1000h
mov cx,200h
mov al,0
repe scasb
jne failed
push es
xor ax,ax
mov es,ax
cmp byte [es:54dh],40h ; color, LOWER, bank zero
jne failed
test byte [es:54ch],80h
jz failed
pop es
in al,0a4h
cmp al,0
jne failed
in al,0a6h
cmp al,0
jne failed
; Identity colors through the existing digital palette readback interface.
in al,0a8h
cmp al,57h
jne failed
in al,0aah
cmp al,13h
jne failed
in al,0ach
cmp al,46h
jne failed
in al,0aeh
cmp al,02h
jne failed
dec bp
jnz .repeat
mov dx,passed
call print
mov al,0
jmp exit
gap:
mov al,0a5h
repe scasb
jne failed
ret
failed:
cld
push cs
pop ds
mov dx,failure
call print
mov al,1
exit:
mov [result],al
mov dx,[oldoff]
mov ax,[oldseg]
mov ds,ax
mov ax,2566h
int 21h
mov al,[cs:result]
mov ah,4ch
int 21h
print:
mov ah,9
int 21h
ret
oldoff: dw 0
oldseg: dw 0
savedss: dw 0
savedsp: dw 0
workseg: dw 0
result: db 0
passed: db 'GINIT CONTRACT PASS',13,10,'$'
failure: db 'GINIT CONTRACT FAILED',13,10,'$'
align 16
times 14 db 0
before: dw 05aa5h
work: times 1200h db 0
after: dw 0a55ah
