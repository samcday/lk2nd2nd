#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int, c_uint, c_void};

#[repr(C)]
#[derive(Debug)]
pub struct extlinux_label {
    pub kernel: *const c_char,
    pub initramfs: *const c_char,
    pub dtb: *const c_char,
    pub dtbdir: *const c_char,
    pub dtboverlays: *mut *const c_char,
    pub cmdline: *const c_char,
}

impl Default for extlinux_label {
    fn default() -> Self {
        Self {
            kernel: 0 as _,
            initramfs: 0 as _,
            dtb: 0 as _,
            dtbdir: 0 as _,
            dtboverlays: 0 as _,
            cmdline: 0 as _,
        }
    }
}

extern "C" {
    pub fn extlinux_parse_conf(data: *mut c_char, size: c_uint, label: *mut extlinux_label) -> c_int;
    pub fn extlinux_expand_conf(label: *mut extlinux_label, root: *const c_char) -> bool;
    pub fn extlinux_boot_label(label: *mut extlinux_label);
}
