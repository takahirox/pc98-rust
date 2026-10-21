/* Original DOS entry and harness; MIT, pc98-rust contributors. */
.arch i186
.code16
.section .start,"ax"
.global _start
_start:
    pushw %cs
    popw %ds
    pushw %cs
    popw %es
    cld
    xorw %ax, %ax
    movw $__bss_start, %di
    movw $__bss_size, %cx
    rep stosb
    call se86_init
    call se86_play_beep
    call se86_play_beep2
    movw $message, %dx
    movb $9, %ah
    int $0x21
    movw $0x4c00, %ax
    int $0x21
message:
    .ascii "INDEPENDENT FM ROUTINES OK\r\n$"
