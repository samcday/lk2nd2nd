use alloc::ffi::CString;
use crate::lk_list::{list_node, LkList};
use core::ffi::{c_char, c_int, c_long, c_longlong, c_uint, c_ulong, c_ulonglong, c_void, CStr};
use core::ops::Deref;
use fatfs::{IoBase, Read, ReadWriteSeek, Seek, SeekFrom, Write};
use crate::println;

#[repr(C)]
#[derive(Debug)]
pub struct LkBlockDev { // bdev_t
    node: list_node,
    _ref: c_int,

    /* info about the block device */
    name: *mut c_char,
    pub size: c_longlong,
    block_size: c_ulong,
    block_count: c_uint,
    label: *mut c_char,
    is_leaf: bool,
}

impl LkBlockDev {
    pub fn label(&self) -> Option<&CStr> {
        if self.label.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(self.label)) }
        }
    }

    pub fn name(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.name) }
    }
}

pub struct OpenDevice {
    dev: *mut LkBlockDev,
    read_pos: c_longlong,
    size: c_longlong,
}

impl Drop for OpenDevice {
    fn drop(&mut self) {
        unsafe { sys::bio_close(self.dev); }
    }
}

impl IoBase for OpenDevice { type Error = (); }

impl Read for OpenDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // NOTE: explicitly not doing bounds checks here, as they're already done in bio_read()
        let read = unsafe { sys::bio_read(self.dev, buf.as_mut_ptr() as _, self.read_pos, buf.len() as c_ulong) };
        if read < 0 {
            Err(())
        } else {
            self.read_pos += read as c_longlong;
            Ok(read as usize)
        }
    }
}

impl Write for OpenDevice {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        println!("unhandled write");
        Err(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // seems that all writes are immediately flushed in bio land.
        Ok(())
    }
}

impl Seek for OpenDevice {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Start(pos) => self.read_pos = pos as c_longlong,
            SeekFrom::End(off) => self.read_pos = self.size + off as c_longlong,
            SeekFrom::Current(off) => self.read_pos += off as c_longlong,
        }
        Ok(self.read_pos as u64)
    }
}


#[repr(C)]
#[derive(Copy, Clone)]
pub struct bdev_struct {
    pub list: list_node,
}

// TODO: static lifetime is wrong
// This should be wrapped in a struct that locks the mutex and unlocks it on Drop
pub fn get_bdevs() -> LkList<'static, LkBlockDev> {
    let bdevs = unsafe { sys::bio_get_bdevs() };
    unsafe { LkList::new(&mut (*bdevs).list) }
}

pub  fn open(name: &CStr) -> Option<OpenDevice> {
    let dev = unsafe { sys::bio_open(name.as_ptr()) };

    return if dev.is_null() {
        None
    } else {
        let dev_ref = unsafe { &mut *dev };
        Some(OpenDevice {
            dev,
            read_pos: 0,
            size: dev_ref.size,
        })
    }
}

mod sys {
    use crate::bio::*;
    extern "C" {
        pub fn bio_get_bdevs() -> *mut bdev_struct;
        pub fn bio_open(name: *const c_char) -> *mut LkBlockDev;
        pub fn bio_close(dev: *mut LkBlockDev);
        pub fn bio_read(dev: *mut LkBlockDev, buf: *mut c_void, offset: c_longlong, len: c_ulong) -> c_long;
    }
}
