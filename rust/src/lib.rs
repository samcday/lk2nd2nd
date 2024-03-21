#![no_std]

use core::ffi::c_char;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

static HELLO: &[u8] = b"Rust says hello!\n\0";

extern "C" {
    pub fn _dputs(str: *const c_char) -> i32;
}

#[no_mangle]
pub unsafe extern "C" fn rust_hello_world() {
    _dputs(HELLO.as_ptr() as *const c_char);
}
