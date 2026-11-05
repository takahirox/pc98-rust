; SPDX-License-Identifier: MIT
; Original DOS allocator diagnostic, Issue #132. No host memory counters.
; Keep only 256 paragraphs, including PSP/code/stack; release the COM tail.
; AH=48/FFFF must fail with AX=8/BX=largest. Then actually allocate that
; block, touch both ends, free it and require an identical second query.
bits 16
cpu 8086
org 0x100
    push cs
    pop ds
    cli
    mov sp, 0x1000
    sti
    mov bx, 0x100
    mov ah, 0x4a
    int 0x21
    jc fail
    call query
    mov [paragraphs], bx
    mov ah, 0x48
    int 0x21
    jc fail
    mov dx, ax
    add dx, [paragraphs]
    jc fail
    cmp dx, 0xa000              ; Entire allocation must be conventional RAM
    ja fail
    mov es, ax
    mov byte [es:0], 0x5a
    mov ax, [paragraphs]
    dec ax
    mov dx, es
    add ax, dx
    mov es, ax
    mov byte [es:15], 0xa5
    cmp byte [es:15], 0xa5
    jne fail
    sub ax, [paragraphs]
    inc ax
    mov es, ax
    cmp byte [es:0], 0x5a
    jne fail
    mov ah, 0x49
    int 0x21
    jc fail
    call query
    cmp bx, [paragraphs]
    jne fail
    mov dx, label_paras
    call puts
    mov ax, bx
    xor dx, dx
    call decimal
    mov dx, label_bytes
    call puts
    mov ax, [paragraphs]
    mov bx, 16
    mul bx
    call decimal
    mov dx, restored
    call puts
    mov ax, 0x4c00
    int 0x21
query:
    mov bx, 0xffff
    mov ah, 0x48
    int 0x21
    jnc fail
    cmp ax, 8
    jne fail
    cmp bx, 1
    jb fail
    ret
puts:
    mov ah, 9
    int 0x21
    ret
; Unsigned DX:AX decimal, using two 16-bit divisions per digit.
decimal:
    xor cx, cx
    mov bx, 10
.next:
    push ax
    mov ax, dx
    xor dx, dx
    div bx
    mov si, ax
    pop ax
    div bx
    push dx
    inc cx
    mov dx, si
    mov di, ax
    or di, dx
    jnz .next
.print:
    pop dx
    add dl, '0'
    mov ah, 2
    int 0x21
    loop .print
    ret
fail:
    mov dx, failed
    call puts
    mov ax, 0x4c01
    int 0x21
paragraphs dw 0
label_paras db 13,10,'MEMSIZE paragraphs=$'
label_bytes db ' bytes=$'
restored db ' allocated/freed/restored OK',13,10,'$'
failed db 13,10,'MEMSIZE FAILED',13,10,'$'
