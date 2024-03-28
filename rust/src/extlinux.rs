use alloc::{format, vec};
use alloc::ffi::CString;
use alloc::string::String;
use anyhow::{ensure, Error, Context};
use crate::lk_fs;
use crate::lk_fs::LkFile;

pub fn scan(partition: &str) -> anyhow::Result<()> {
    let mountpoint = format!("/{}", partition);
    lk_fs::mount(&mountpoint, "ext2", partition).context("ext2 mount failed")?;
    let file = LkFile::open(&format!("{}/extlinux/extlinux.conf", mountpoint)).map_err(Error::msg).context("open extlinux.conf failed")?;
    let (_, size) = file.stat().map_err(Error::msg).context("stat extlinux.conf failed")?;
    let mut data = vec![0; size + 1];
    file.read(&mut data[..size], 0).context("read extlinux.conf failed")?;

    let mut label: sys::extlinux_label = Default::default();
    let ret = unsafe { sys::extlinux_parse_conf(data.as_mut_ptr() as _, data.len() as _, &mut label) };
    ensure!(ret >= 0, "parsing extlinux.conf failed");

    let root = CString::new(mountpoint.clone()).map_err(Error::msg).context("mountpoint is invalid UTF-8?!")?;
    let ret = unsafe { sys::extlinux_expand_conf(&mut label, root.as_ptr()) };
    ensure!(ret, "expanding extlinux.conf failed");

    unsafe { sys::extlinux_boot_label(&mut label); }
}

mod sys {
    #![allow(non_camel_case_types)]

    use core::ffi::{c_char, c_int, c_uint};

    #[repr(C)]
    #[derive(Debug)]
    pub struct extlinux_label {
        pub label: *const c_char,
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
                label: 0 as _,
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
        pub fn extlinux_boot_label(label: *mut extlinux_label) -> !;
    }
}