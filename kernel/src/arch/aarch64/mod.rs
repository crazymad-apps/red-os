// AArch64 architecture-specific code

use core::arch::global_asm;

// Include the assembly boot code
global_asm!(include_str!("boot.S"));
