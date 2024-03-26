use core::ffi::c_char;

extern "C" {
    pub fn _dputs(str: *const c_char) -> i32;
    pub fn _dputc(c: c_char);
}

#[macro_export]
macro_rules! println {
    () => {
        unsafe { $crate::fmt::_dputc('\n' as core::ffi::c_char); }
    };
    ($($arg:tt)*) => {{
        unsafe {
            let str = alloc::ffi::CString::new(alloc::format!($($arg)*).as_str()).unwrap();
            $crate::fmt::_dputs(str.as_ptr());
            $crate::fmt::_dputc('\n' as core::ffi::c_char);
        }
    }};
}
