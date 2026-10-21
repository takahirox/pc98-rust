OUTPUT_FORMAT(binary)
SECTIONS {
  . = 0x100;
  .text : { *(.start) *(.text*) }
  .rodata : { *(.rodata*) }
  .data : { *(.data*) }
  __bss_start = .;
  .bss : { *(.bss*) *(COMMON) }
  __bss_size = . - __bss_start;
  /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
