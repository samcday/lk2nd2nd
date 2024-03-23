#![no_std]

mod lk_alloc;

extern crate alloc;

use alloc::vec;
use core::panic::PanicInfo;
use crate::lk_alloc::LkHeap;

#[global_allocator]
static ALLOCATOR: LkHeap = LkHeap;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

extern "C" {
    pub fn _dputs(str: *const u8) -> i32;
}

#[no_mangle]
pub unsafe extern "C" fn rust_hello_world() {
    let hi = vec!["Rust", "says", "hello!"];

    let mut str = alloc::string::String::new();

    for v in hi {
        str.push_str(v);
        str.push(' ');
    }
    str.push('\n');

    _dputs(str.as_ptr());
}
