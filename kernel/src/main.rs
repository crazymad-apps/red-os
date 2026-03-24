#![no_std]
#![no_main]
mod arch;
mod uart;

use core::panic::PanicInfo;

/// Kernel entry point, called from boot.S after stack and BSS are set up.
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    println!("Hello, Red OS!");
    println!("Architecture: AArch64");
    println!("Kernel loaded and running.");

    loop {
        // Halt CPU using WFI (Wait For Interrupt)
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("\n[KERNEL PANIC]");
    if let Some(loc) = info.location() {
        println!("  at {}:{}", loc.file(), loc.line());
    }
    if let Some(msg) = info.message().as_str() {
        println!("  {}", msg);
    }
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
