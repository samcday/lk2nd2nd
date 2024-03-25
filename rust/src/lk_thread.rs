use alloc::ffi::CString;
use core::ffi::c_ulong;
use core::ptr::null_mut;
use core::time::Duration;

pub fn spawn(name: &str, f: fn() -> !) -> bool {
    let name = match CString::new(name) {
        Ok(name) => name.as_ptr(),
        Err(_) => return false
    };

    let thr = unsafe {
        sys::thread_create(name, core::intrinsics::transmute(f as *const fn()), null_mut(), sys::DEFAULT_PRIORITY, sys::DEFAULT_STACK_SIZE)
    };
    if thr.is_null() {
        return false;
    }
    unsafe { sys::thread_resume(thr); }
    return true;
}

pub fn exit() -> ! {
    unsafe { sys::thread_exit(0) }
}

pub fn sleep(dur: Duration) {
    unsafe { sys::thread_sleep(dur.as_millis() as c_ulong); }
}

mod sys {
    #![allow(non_camel_case_types)]
    use core::ffi::{c_char, c_int, c_ulong, c_void};

    const NUM_PRIORITIES: c_int = 32;
    pub const DEFAULT_PRIORITY: c_int = NUM_PRIORITIES / 2;
    pub const DEFAULT_STACK_SIZE: usize = 12288;

    pub type thread_start_routine = ::core::option::Option<
        unsafe extern "C" fn(arg: *mut ::core::ffi::c_void) -> ::core::ffi::c_int,
    >;

    extern "C" {
        pub fn thread_create(name: *const c_char, entry: thread_start_routine, arg: *mut c_void, priority: c_int, stack_size: usize) -> *mut c_void;
        pub fn thread_resume(arg1: *mut c_void) -> c_int;
        pub fn thread_sleep(delay: c_ulong);
        pub fn thread_exit(code: c_int) -> !;
    }
}
