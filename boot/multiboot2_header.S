/* Stub educacional Multiboot2 — NÃO linkado pelo build deste MVP.
 * Magic / arch / length / checksum no formato clássico. */
.set MAGIC,    0xE85250D6
.set ARCH,     0          /* i386 protected mode (histórico); x86_64 usa Limine na prática */
.set LENGTH,   header_end - header_start
.set CHECKSUM, -(MAGIC + ARCH + LENGTH)

.section .multiboot_header, "a"
.align 8
header_start:
    .long MAGIC
    .long ARCH
    .long LENGTH
    .long CHECKSUM
    /* end tag */
    .word 0
    .word 0
    .long 8
header_end:
