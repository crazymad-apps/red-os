/* AArch64 Kernel Boot Code */
/* This is the very first code executed after QEMU loads the kernel */

.section ".text.boot"
.global _start

_start:
    /* Only let CPU 0 proceed, park others */
    mrs     x0, mpidr_el1
    and     x0, x0, #0xFF
    cbnz    x0, .park

    /* Disable all interrupts */
    msr     daifset, #0xF

    /* Set up stack pointer (grows downward from _start) */
    adr     x0, _start
    mov     sp, x0

    /* Clear BSS section */
    adr     x0, __bss_start
    adr     x1, __bss_end
.clear_bss:
    cmp     x0, x1
    b.ge    .bss_done
    str     xzr, [x0], #8
    b       .clear_bss
.bss_done:

    /* Jump to Rust kernel_main */
    bl      kernel_main

    /* Should never return, but park if it does */
.park:
    wfe
    b       .park
