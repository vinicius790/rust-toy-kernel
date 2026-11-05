#![no_std]
#![no_main]

use core::arch::asm;
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use limine::request::{RequestsEndMarker, RequestsStartMarker};
use limine::BaseRevision;

const COM1: u16 = 0x3f8;
const SERIAL_TIMEOUT: usize = 1_000_000;

#[used]
#[link_section = ".limine_requests_start"]
static REQUESTS_START: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[link_section = ".limine_requests"]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
#[link_section = ".limine_requests_end"]
static REQUESTS_END: RequestsEndMarker = RequestsEndMarker::new();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_init();
    if !BASE_REVISION.is_supported() || !BASE_REVISION.is_valid() {
        serial_write(b"BOOT_FAIL: unsupported Limine base revision\n");
        serial_flush();
        halt()
    }
    serial_write(b"BOOT_OK: toy-kernel booted with Limine (x86_64)\n");
    serial_flush();
    halt()
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    serial_write(b"PANIC: ");
    let _ = write!(SerialWriter, "{}", _info.message());
    if let Some(location) = _info.location() {
        let _ = write!(
            SerialWriter,
            " at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
    }
    serial_write(b"\n");
    serial_flush();
    halt()
}

struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        serial_write(text.as_bytes());
        Ok(())
    }
}

fn serial_init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1, 0x03);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xc7);
        outb(COM1 + 4, 0x0b);
    }
}

fn serial_write(bytes: &[u8]) {
    for &byte in bytes {
        if byte == b'\n' {
            serial_write_byte(b'\r');
        }
        serial_write_byte(byte);
    }
}

fn serial_write_byte(byte: u8) {
    for _ in 0..SERIAL_TIMEOUT {
        if unsafe { inb(COM1 + 5) } & 0x20 != 0 {
            unsafe { outb(COM1, byte) };
            return;
        }
    }
}

fn serial_flush() {
    for _ in 0..SERIAL_TIMEOUT {
        if unsafe { inb(COM1 + 5) } & 0x40 != 0 {
            return;
        }
    }
}

unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("cli; hlt", options(nomem, nostack));
        }
    }
}
