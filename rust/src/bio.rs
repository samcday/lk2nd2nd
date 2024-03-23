use crate::lk_list::{list_node, LkList};
use core::ffi::{c_char, c_int, c_longlong};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct bdev {
    node: list_node,
    pub _ref: c_int,

    /* info about the block device */
    pub name: *mut c_char,
    pub size: c_longlong,
    // block_size: c_uint, // size_t
    // block_count: c_uint,
    // label: *const c_char,
    // is_leaf: bool,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct bdev_struct {
    pub list: list_node,
    // mutex_t lock;
}

extern "C" {
    pub fn bio_get_bdevs() -> *mut bdev_struct;
}

// TODO: static lifetime is wrong
// This should be wrapped in a struct that locks the mutex and unlocks it on Drop
pub fn get_bdevs() -> LkList<'static, bdev> {
    let bdevs = unsafe { bio_get_bdevs() };
    unsafe { LkList::new(&mut (*bdevs).list) }
}
