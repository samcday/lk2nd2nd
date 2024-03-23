#![no_std]

mod bio;
mod lk_alloc;
mod lk_list;

extern crate alloc;

use crate::lk_alloc::LkHeap;
use alloc::{format, vec};
use core::ffi::CStr;

use core::panic::PanicInfo;

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

    let mut output = alloc::string::String::new();
    for v in hi {
        output.push_str(v);
        output.push(' ');
    }
    output.push('\n');

    for dev in bio::get_bdevs() {
        if let Ok(str) = CStr::from_ptr(dev.name).to_str() {
            output.push_str(format!("dev {} is {}\n", str, dev.size).as_str());
        }
    }

    output.push('\0');
    _dputs(output.as_ptr());
}
