; SPDX-License-Identifier: MIT
; Original guest XMS/HMA functional test, Issue #132. Real INT 2F + driver calls.
; Never write to DOS-owned HMA or toggle A20 underneath DOS/interrupt handlers.
bits 16
cpu 386
org 0x100
    push cs
    pop ds
    mov byte [stage], 'A'
    mov ax, 0x3306
    int 0x21
    test dh, 0x10                 ; DOS reports resident kernel in HMA
    jz fail
    mov ax, 0x4300
    int 0x2f
    cmp al, 0x80
    jne fail
    mov ax, 0x4310
    int 0x2f
    mov [entry], bx
    mov [entry+2], es
    mov byte [stage], 'B'
    mov ah, 7                     ; Query physical A20 through driver
    call far [entry]
    cmp ax, 1
    jne fail
    mov byte [stage], 'C'
    mov ah, 5                     ; Balanced local A20 lock/unlock
    call far [entry]
    cmp ax, 1
    jne fail
    mov byte [stage], 'D'
    mov ah, 6
    call far [entry]
    test ax, ax
    jnz fail
    cmp bl, 0x94                  ; Balanced unlock; DOS still holds A20
    jne fail
    mov byte [stage], 'E'
    mov ah, 7
    call far [entry]
    cmp ax, 1                     ; DOS's existing HMA lock remains
    jne fail
    mov byte [stage], 'F'
    mov ah, 1
    mov dx, 0xffff                ; HMA must already belong to DOS
    call far [entry]
    test ax, ax
    jnz fail
    cmp bl, 0x91                  ; HMA already allocated
    jne fail
    mov byte [stage], 'G'
    mov ah, 8
    call far [entry]
    mov [free_before], dx
    mov ah, 9
    mov dx, 16                    ; A real 16 KiB XMS allocation
    call far [entry]
    cmp ax, 1
    jne fail
    mov [handle], dx
    mov [to_xms+10], dx
    mov [from_xms+4], dx
    mov ax, ds
    mov [to_xms+8], ax
    mov [from_xms+14], ax
    mov si, to_xms
    mov byte [stage], 'H'
    mov ah, 0x0b
    call far [entry]
    cmp ax, 1
    jne fail
    mov si, from_xms
    mov byte [stage], 'I'
    mov ah, 0x0b
    call far [entry]
    cmp ax, 1
    jne fail
    cmp dword [dest], 0x78563412
    jne fail
    cmp dword [dest+4], 0x9abcdef0
    jne fail
    mov dword [from_xms+6], 16384  ; Offset beyond allocated block
    mov si, from_xms
    mov byte [stage], 'J'
    mov ah, 0x0b
    call far [entry]
    test ax, ax
    jnz fail
    cmp bl, 0xa7                  ; HimemX rejects offset+length beyond block
    jne fail
    cmp dword [dest], 0x78563412   ; Rejected move left destination intact
    jne fail
    mov word [from_xms+4], 0xffff ; Invalid source handle
    mov dword [from_xms+6], 0
    mov si, from_xms
    mov byte [stage], 'K'
    mov ah, 0x0b
    call far [entry]
    test ax, ax
    jnz fail
    cmp bl, 0xa3
    jne fail
    mov dx, [handle]
    mov [from_xms+4], dx
    mov byte [stage], 'L'
    mov ah, 0x0c                  ; Lock, require an extended physical address
    call far [entry]
    cmp ax, 1
    jne fail
    cmp dx, 0x10
    jb fail
    mov dx, [handle]
    mov byte [stage], 'M'
    mov ah, 0x0d
    call far [entry]
    cmp ax, 1
    jne fail
    mov dword [dest], 0
    mov si, from_xms              ; Valid move after both rejected requests
    mov byte [stage], 'N'
    mov ah, 0x0b
    call far [entry]
    cmp ax, 1
    jne fail
    cmp dword [dest], 0x78563412
    jne fail
    cmp dword [dest+4], 0x9abcdef0
    jne fail
    mov dx, [handle]
    mov byte [stage], 'O'
    mov ah, 0x0a
    call far [entry]
    cmp ax, 1
    jne fail
    mov byte [stage], 'P'
    mov ah, 8
    call far [entry]
    cmp dx, [free_before]
    jne fail
    mov dx, passed
    mov ah, 9
    int 0x21
    mov ax, 0x4c00
    int 0x21
fail:
    mov dl, [stage]
    mov ah, 2
    int 0x21
    mov dx, failed
    mov ah, 9
    int 0x21
    mov ax, 0x4c01
    int 0x21
stage db 0
entry dd 0
handle dw 0
free_before dw 0
to_xms:
    dd 8
    dw 0
    dw source, 0
    dw 0
    dd 0
from_xms:
    dd 8
    dw 0
    dd 0
    dw 0
    dw dest, 0
source dd 0x78563412, 0x9abcdef0
dest dd 0, 0
passed db 13,10,'XMSCHECK HMA/A20/move/invalid/recovery/free OK',13,10,'$'
failed db 13,10,'XMSCHECK FAILED',13,10,'$'
