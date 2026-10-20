; STAR TRAIL -- original bootable PC-98 game, MIT (see ../LICENSE).
; Only x86 guest instructions draw the board, handle input, and save media.
bits 16
cpu 186
org 0
jmp loader
db 'STAR98  '
loader:
cli
xor ax, ax
mov ss, ax
mov sp, 0x7c00
push cs
pop ds
push cs
pop es
mov bp, 1024
mov bx, 6144
mov cx, 0x0300
mov dx, 2
mov ax, 0x5690
int 0x1b
jc fail
jmp start
fail: hlt
jmp fail
times 1024-($-$$) db 0
start:
sti
cld
push cs
pop ds
mov ax, 0xa000
mov es, ax
xor di, di
mov ax, 32
mov cx, 2000
rep stosw
; Bitmap blue star field beneath text, through the slave GDC planar window.
mov ax, 0xa800
mov es, ax
xor di, di
mov cx, 32000
xor al, al
rep stosb
mov di, 223
mov cx, 120
stars:
mov byte [es:di], 0x08
add di, 263
loop stars
; Load the previous run's save record from track 0, head 0, sector 8.
push cs
pop es
mov bp, save
mov bx, 1024
mov cx, 0x0300
mov dx, 8
mov ax, 0x5690
int 0x1b
mov si, title
mov di, 164
call print
mov si, help
mov di, 484
call print
mov si, goal
mov di, 644
call print
mov si, score_label
mov di, 2884
call print
mov si, credit
mov di, 3524
call print
mov byte [x], 1
mov byte [y], 1
call draw
main:
mov ah, 0x18
int 0x18 ; beep OFF after one key interval
xor ah, ah
int 0x18 ; blocks through keyboard IRQ and BIOS FIFO
cmp ah, 0x13 ; R
je restart
cmp byte [won], 1
je main
mov bl, [x]
mov bh, [y]
cmp ah, 0x3a
jb main
cmp ah, 0x3d
ja main
cmp ah, 0x3a ; up
jne .down
dec bh
.down:
cmp ah, 0x3d
jne .left
inc bh
.left:
cmp ah, 0x3b
jne .right
dec bl
.right:
cmp ah, 0x3c
jne .check
inc bl
.check:
cmp bl, 1
jb main
cmp bl, 10
ja main
cmp bh, 1
jb main
cmp bh, 6
ja main
; Obstacles create a small maze in the middle.
cmp bl, 5
jne .move
cmp bh, 3
jb main
cmp bh, 5
ja main
.move:
mov [x], bl
mov [y], bh
inc word [moves]
mov al, bl
mov dl, bh
mov si, gems
mov cx, 3
.gems:
cmp al, [si]
jne .next
cmp dl, [si+1]
jne .next
mov byte [si], 0xff
inc byte [score]
mov ah, 0x17
int 0x18
.next:
add si, 2
loop .gems
cmp byte [score], 3
jne .redraw
mov byte [won], 1
mov si, victory
mov di, 3204
call print
.redraw:
call draw
; Save move count and score with a PC-98 BIOS WRITE, through FDC and DMA.
mov ax, [moves]
mov [save+8], ax
mov al, [score]
mov [save+10], al
push cs
pop es
mov bp, save
mov bx, 1024
mov cx, 0x0300
mov dx, 8
mov ax, 0x5590
int 0x1b
jc fail
mov cx, 15000
.tone: loop .tone
jmp main
restart:
mov byte [score], 0
mov word [moves], 0
mov byte [won], 0
mov word [gems], 0x0104
mov word [gems+2], 0x0608
mov word [gems+4], 0x030a
mov byte [x], 1
mov byte [y], 1
mov si, blank
mov di, 3204
call print
call draw
jmp main
; ES:A000 print a NUL-terminated ASCII string at byte offset DI.
print:
push ax
push es
mov ax, 0xa000
mov es, ax
.print:
lodsb
or al, al
jz .done
xor ah, ah
stosw
jmp .print
.done:
pop es
pop ax
ret
; Eight by twelve board: border plus 10x6 interior.
draw:
pusha
mov ax, 0xa000
mov es, ax
mov di, 1130
xor dx, dx
.row:
xor bx, bx
.cell:
mov ax, '.'
cmp dl, 0
je .wall
cmp dl, 7
je .wall
cmp bl, 0
je .wall
cmp bl, 11
je .wall
cmp bl, 5
jne .gem
cmp dl, 3
jb .wall
cmp dl, 5
jbe .gem
.wall:
mov ax, '#'
.gem:
mov si, gems
mov cx, 3
.find:
cmp bl, [si]
jne .skip
cmp dl, [si+1]
jne .skip
mov ax, '*'
.skip:
add si, 2
loop .find
cmp bl, [x]
jne .put
cmp dl, [y]
jne .put
mov ax, '@'
.put:
stosw
inc bl
cmp bl, 12
jb .cell
add di, 136
inc dl
cmp dl, 8
jb .row
mov al, [score]
add al, '0'
mov [es:2898], al
popa
ret

title db 'STAR TRAIL / PC-9801',0
help db 'ARROWS: MOVE   R: RESTART',0
goal db 'COLLECT THREE STARS *. YOU ARE @.',0
score_label db 'STARS: 0 / 3',0
credit db 'ORIGINAL MIT GUEST GAME / NO DOS OR ROM REQUIRED',0
victory db 'YOU WIN! PRESS R TO PLAY AGAIN.',0
blank db '                              ',0
gems db 4,1,8,6,10,3
; Public test/debug record. The host only reads this; the guest owns all updates.
times 0x1800-($-$$) db 0
db 'PC98GAME'
x db 0
y db 0
score db 0
db 0
moves dw 0
won db 0
times 0x1c00-($-$$) db 0
save db 'STAR98SV'
times 0x2000-($-$$) db 0
