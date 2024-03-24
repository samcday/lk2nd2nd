#![no_std]

extern crate alloc;

use core::ffi::c_char;
use crate::lk_app::app_descriptor;

mod fmt;
mod lk_alloc;
mod panic;
mod lk_app;

unsafe extern "C" fn app_init(_: *const app_descriptor) {
    println!("HOLY SHIT");
}

#[link_section = ".apps"]
static app: app_descriptor = app_descriptor {
    name: &unsafe { *c"rust_test".as_ptr() },
    init: Some(app_init),
    entry: None,
    flags: 0,
};
