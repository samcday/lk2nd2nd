use core::ffi::c_void;
use core::marker::PhantomData;

#[repr(C)]
pub struct Mutex {
    _data: [u8; 0],
    _marker: PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

pub struct MutexGuard {
    mutex: *mut Mutex,
}

impl Drop for MutexGuard {
    fn drop(&mut self) {
        unsafe { sys::mutex_release(self.mutex); }
    }
}

pub fn acquire(mutex: *mut Mutex) -> Result<MutexGuard, ()> {
    if mutex.is_null() {
        return Err(());
    }

    let ret = unsafe { sys::mutex_acquire(mutex) };
    if ret != 0 {
        return Err(());
    }
    Ok(MutexGuard{mutex})
}

mod sys {
    use core::ffi::c_int;
    use crate::lk_mutex::Mutex;

    extern "C" {
        pub fn mutex_acquire(m: *mut Mutex) -> c_int;
        pub fn mutex_release(m: *mut Mutex) -> c_int;
    }
}
