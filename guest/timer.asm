; Original MIT interval callback probe for Issue #14, based on Issue #13 timer.asm.
; Executed by the real FreeDOS kernel and FreeCOM, with no host DOS service.
bits 16
org 100h
push cs
pop ds
push cs
pop es
mov [expected_sp],sp
mov [guest_segment],cs
mov bx,callback
mov cx,1
mov ah,2
int 1ch
sti
mov cx,0ffffh
wait_start:
cmp byte [fired],0
jne passed
loop wait_start
jmp fail
wait_end:
passed:
; Validate the IRQ frame captured by guest code as well as callback execution.
cmp byte [fired],1
jne fail
mov ax,[expected_sp]
sub ax,6
cmp ax,[entry_sp]
jne fail
cmp word [return_ip],wait_start
jb fail
cmp word [return_ip],wait_end
jae fail
mov ax,cs
cmp ax,[return_cs]
jne fail
test word [return_flags],0200h
jz fail
mov byte [result],1
mov dx,ok
jmp print
fail:
mov dx,failed
print:
mov ax,0900h
int 21h
; Remain resident in this bounded observation loop for native/Wasm assertions.
; No further request: stepping for many intervals must leave fired equal to 1.
sti
idle:
hlt
jmp idle
callback:
push bp
mov bp,sp
push ax
lea ax,[bp+2]
mov [cs:entry_sp],ax
mov ax,[ss:bp+2]
mov [cs:return_ip],ax
mov ax,[ss:bp+4]
mov [cs:return_cs],ax
mov ax,[ss:bp+6]
mov [cs:return_flags],ax
inc byte [cs:fired]
pop ax
pop bp
iret
failed: db 'TIMER CALLBACK TIMEOUT',13,10,'$'
ok: db 'TIMER CALLBACK PASS',13,10,'$'
times 200h-($-$$) db 0
; Fixed state offsets from COM segment: 0300h signature, 0308h count, etc.
state: db 'T98STATE'
fired: db 0
result: db 0
expected_sp: dw 0
entry_sp: dw 0
return_ip: dw 0
return_cs: dw 0
return_flags: dw 0
guest_segment: dw 0
wait_bounds: dw wait_start,wait_end
