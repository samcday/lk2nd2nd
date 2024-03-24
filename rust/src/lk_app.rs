use core::ffi::c_uint;

pub type app_init = Option<unsafe extern "C" fn(arg1: *const app_descriptor)>;
pub type app_entry = Option<
    unsafe extern "C" fn(arg1: *const app_descriptor, args: *mut ::core::ffi::c_void),
>;

#[repr(C)]
struct app_descriptor {
    name: *const char,
    init: app_init,
    entry: app_entry,
    flags: c_uint,
}
