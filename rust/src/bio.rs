use alloc::ffi::CString;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_long, c_longlong, c_uint, c_ulong, c_void, CStr};

use crate::lk_list::{list_node, LkListIterator};
use crate::lk_mutex::{acquire, Mutex, MutexGuard};
use crate::println;
use fatfs::{IoBase, Read, Seek, SeekFrom, Write};

#[derive(Clone, Debug)]
pub struct BlockDev {
    pub name: String,
    pub size: usize,
    pub block_size: u64,
    pub block_count: u32,
    pub label: Option<String>,
    pub is_leaf: bool,
}

pub struct OpenDevice {
    dev: *mut sys::bdev_t,
    read_pos: c_longlong,
    size: c_longlong,
}

impl Drop for OpenDevice {
    fn drop(&mut self) {
        unsafe {
            sys::bio_close(self.dev);
        }
    }
}

impl IoBase for OpenDevice {
    type Error = ();
}

impl Read for OpenDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // NOTE: explicitly not doing bounds checks here, as they're already done in bio_read()
        let read = unsafe {
            sys::bio_read(
                self.dev,
                buf.as_mut_ptr() as _,
                self.read_pos,
                buf.len() as c_ulong,
            )
        };
        if read < 0 {
            Err(())
        } else {
            self.read_pos += read as c_longlong;
            Ok(read as usize)
        }
    }
}

impl Write for OpenDevice {
    fn write(&mut self, _buf: &[u8]) -> Result<usize, Self::Error> {
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

// pub struct BlockDevIterator<'a> {
//     iter: LkListIterator<'a, &'a mut LkBlockDev>,
//     _guard: MutexGuard,
// }
//
// impl<'a> Iterator for BlockDevIterator<'a> {
//     type Item = &'a mut LkBlockDev;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         self.iter.next()
//     }
// }



// impl LkBlockDev {
//     pub fn label(&self) -> Option<&CStr> {
//         if self.label.is_null() {
//             None
//         } else {
//             unsafe { Some(CStr::from_ptr(self.label)) }
//         }
//     }
//
//     pub fn name(&self) -> &CStr {
//         unsafe { CStr::from_ptr(self.name) }
//     }
// }

pub fn get_bdevs() -> Result<Vec<BlockDev>, ()> {
    let bdevs = unsafe { sys::bio_get_bdevs().as_mut() }.ok_or(())?;
    let _guard = acquire(&mut bdevs.mutex).map_err(|_| ())?;
    Ok(LkListIterator::<&mut sys::bdev_t>::new(&mut bdevs.list).filter_map(|dev| {
        let (name, label) = unsafe {
            (
                CStr::from_ptr(dev.name).to_str().ok(),
                if dev.label.is_null() { None } else { CStr::from_ptr(dev.label).to_str().map_err(|_| ()).ok().map(|v| v.to_string()) }
            )
        };
        if name.is_none() {
            return None;
        }
        Some(BlockDev {
            name: name.unwrap().to_string(),
            size: dev.size as usize,
            block_size: dev.block_size.into(),
            is_leaf: dev.is_leaf,
            label,
            block_count: dev.block_count,
        })
    }).collect())
}

pub fn open(name: &str) -> Result<OpenDevice, ()> {
    let name = CString::new(name).map_err(|_| ())?;
    let dev = unsafe { sys::bio_open(name.as_ptr()) };

    if dev.is_null() {
        Err(())
    } else {
        let dev_ref = unsafe { &mut *dev };
        Ok(OpenDevice {
            dev,
            read_pos: 0,
            size: dev_ref.size,
        })
    }
}

mod sys {
    #![allow(non_camel_case_types)]

    use core::ffi::{c_char, c_int, c_long, c_longlong, c_uint, c_ulong, c_void};
    use crate::lk_list::list_node;
    use crate::lk_mutex::Mutex;

    #[repr(C)]
    pub struct bdev_struct {
        pub list: list_node,
        pub mutex: Mutex,
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct bdev_t {
        node: list_node,
        _ref: c_int,
        pub name: *mut c_char,
        pub size: c_longlong,
        pub block_size: c_ulong,
        pub block_count: c_uint,
        pub label: *mut c_char,
        pub is_leaf: bool,
    }

    extern "C" {
        pub fn bio_get_bdevs() -> *mut bdev_struct;
        pub fn bio_open(name: *const c_char) -> *mut bdev_t;
        pub fn bio_close(dev: *mut bdev_t);
        pub fn bio_read(
            dev: *mut bdev_t,
            buf: *mut c_void,
            offset: c_longlong,
            len: c_ulong,
        ) -> c_long;
    }
}
