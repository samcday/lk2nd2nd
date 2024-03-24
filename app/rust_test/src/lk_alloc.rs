use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_uint;

pub struct LkHeap;

#[global_allocator]
static ALLOCATOR: LkHeap = LkHeap;

mod sys {
    use core::ffi::{c_uint, c_void};

    extern "C" {
        pub fn heap_alloc(sz: c_uint, alignment: c_uint) -> *mut c_void;
        pub fn heap_free(ptr: *mut c_void);
    }
}

unsafe impl GlobalAlloc for LkHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        sys::heap_alloc(layout.size() as c_uint, layout.align() as c_uint) as _
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        sys::heap_free(ptr as _);
    }
}
