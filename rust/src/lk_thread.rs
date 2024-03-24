struct Thread {
    handle: *mut sys::thread_t,
}

impl Thread {
    pub fn spawn(f: fn() -> !) -> Result<Self, ()> {
        return Err(())
    }
}

mod sys {
    use core::ffi::c_int;

    #[repr(C)]
    pub struct thread_t {
        magic: c_int,
    }
}
