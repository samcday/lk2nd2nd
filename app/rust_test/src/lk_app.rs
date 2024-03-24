use core::ffi::{c_char, c_uint};

pub type app_init = Option<unsafe extern "C" fn(arg1: *const app_descriptor)>;
pub type app_entry = Option<
    unsafe extern "C" fn(arg1: *const app_descriptor, args: *mut ::core::ffi::c_void),
>;

#[repr(C)]
pub struct app_descriptor {
    pub name:  &'static c_char,
    pub init: app_init,
    pub entry: app_entry,
    pub flags: c_uint,
}
