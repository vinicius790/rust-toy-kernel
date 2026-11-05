/* Link the freestanding image in the x86_64 higher half. */
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

KERNEL_VMA = 0xFFFFFFFF80000000;
KERNEL_LOAD_ADDRESS = 0x100000;

SECTIONS
{
    . = KERNEL_VMA + KERNEL_LOAD_ADDRESS;

    .limine_requests_start : AT(ADDR(.limine_requests_start) - KERNEL_VMA) {
        KEEP(*(.limine_requests_start))
    }
    .limine_requests : AT(ADDR(.limine_requests) - KERNEL_VMA) {
        KEEP(*(.limine_requests))
    }
    .limine_requests_end : AT(ADDR(.limine_requests_end) - KERNEL_VMA) {
        KEEP(*(.limine_requests_end))
    }

    . = ALIGN(4K);
    .text : AT(ADDR(.text) - KERNEL_VMA) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .rodata : AT(ADDR(.rodata) - KERNEL_VMA) {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4K);
    .data : AT(ADDR(.data) - KERNEL_VMA) {
        *(.data .data.*)
    }

    . = ALIGN(4K);
    .bss : AT(ADDR(.bss) - KERNEL_VMA) {
        *(.bss .bss.*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame .eh_frame.*)
        *(.comment)
    }
}
