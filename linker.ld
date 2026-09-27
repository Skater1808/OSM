/* Linker Script für den Kernel - linker.ld */
ENTRY(_start)

SECTIONS
{
    . = 0x10000;  /* Kernel wird bei 1MB (0x10000) geladen */

    .text : {
        *(.text._start)
        *(.text*)
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame*)
        *(.comment*)
    }
}