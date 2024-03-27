use alloc::ffi::CString;
use alloc::string::String;
use core::ffi::{c_int, c_uint};
use anyhow::{ensure, Error};
use snafu::Snafu;

#[derive(Debug)]
pub struct LkFile {
    handle: *mut sys::filehandle
}

#[derive(Debug, Snafu)]
pub enum OpenError {
    InvalidPath,
    NoHandle,
    NotFound,
    Unknown{code: c_int}
}

impl LkFile {
    pub fn open(path: &str) -> Result<Self, OpenError> {
        let path = CString::new(path).map_err(|_| OpenError::InvalidPath)?;
        let mut handle: *mut sys::filehandle = 0 as _;
        let ret = unsafe { sys::fs_open_file(path.as_ptr(), &mut handle as *mut _) };
        if ret == -2 {
            return Err(OpenError::NotFound);
        } else if ret < 0 {
            return Err(OpenError::Unknown{code: ret});
        } else if handle.is_null() {
            return Err(OpenError::NoHandle);
        }

        Ok(LkFile{
            handle,
        })
    }

    pub fn read(&self, buf: &mut [u8], offset: i64) -> anyhow::Result<usize> {
        let ret = unsafe { sys::fs_read_file(self.handle, buf.as_mut_ptr() as _, offset, buf.len() as c_uint) };
        ensure!(ret >= 0, "read failed with error {}", ret);
        Ok(ret as usize)
    }

    pub fn stat(&self) -> anyhow::Result<(bool, usize)> {
        let mut stat: sys::file_stat = Default::default();
        let ret = unsafe { sys::fs_stat_file(self.handle, &mut stat as *mut _) };
        ensure!(ret >= 0, "stat failed with error {}", ret);
        Ok((stat.is_dir, stat.size as usize))
    }
}

impl Drop for LkFile {
    fn drop(&mut self) {
        unsafe { sys::fs_close_file(self.handle); }
    }
}

pub fn mount(path: &str, fs: &str, device: &str) -> anyhow::Result<()> {
    let path = CString::new(path).map_err(Error::msg)?;
    let fs = CString::new(fs).map_err(Error::msg)?;
    let device = CString::new(device).map_err(Error::msg)?;
    let ret = unsafe { sys::fs_mount(path.as_ptr(), fs.as_ptr(), device.as_ptr()) };
    // TODO: there are actually useful error codes we could map to an enum here but meh
    ensure!(ret >= 0 || ret == -19, "error code {}", ret);
    Ok(())
}

mod sys {
    #![allow(non_camel_case_types)]

    use core::ffi::{c_char, c_int, c_longlong, c_uint, c_void};
    use core::marker::PhantomData;

    #[repr(C)]
    pub struct filehandle {
        _data: [u8; 0],
        _marker: PhantomData<(*mut u8, core::marker::PhantomPinned)>,
    }
    #[repr(C)]
    #[derive(Debug, Default)]
    pub struct file_stat {
        pub is_dir: bool,
        pub size: c_longlong,
    }

    extern "C" {
        pub fn fs_mount(path: *const c_char, fs: *const c_char, device: *const c_char) -> c_int;
        pub fn fs_open_file(path: *const c_char, handle: *mut *mut filehandle) -> c_int;
        pub fn fs_read_file(
            handle: *mut filehandle,
            buf: *mut c_void,
            offset: c_longlong,
            len: c_uint,
        ) -> isize;

        pub fn fs_close_file(handle: *mut filehandle) -> c_int;
        pub fn fs_stat_file(handle: *mut filehandle, stat: *mut file_stat) -> c_int;
    }
}
