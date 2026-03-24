// PL011 UART driver for QEMU virt machine
// Base address: 0x09000000

const UART_BASE: usize = 0x0900_0000;

// PL011 register offsets
const UART_DR: usize = 0x00;   // Data Register
const UART_FR: usize = 0x18;   // Flag Register
const UART_FR_TXFF: u32 = 1 << 5; // TX FIFO Full

#[inline]
fn uart_reg(offset: usize) -> *mut u32 {
    (UART_BASE + offset) as *mut u32
}

/// Write a single byte to UART, waiting if TX FIFO is full.
pub fn putc(c: u8) {
    unsafe {
        // Wait until TX FIFO has space
        while uart_reg(UART_FR).read_volatile() & UART_FR_TXFF != 0 {}
        uart_reg(UART_DR).write_volatile(c as u32);
    }
}

/// Write a string to UART.
pub fn puts(s: &str) {
    for c in s.bytes() {
        if c == b'\n' {
            putc(b'\r');
        }
        putc(c);
    }
}

/// Print macro support: format and send to UART.
pub struct UartWriter;

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        puts(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        let _ = write!($crate::uart::UartWriter, $($arg)*);
    });
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
