#[panic_handler]
#[cfg(not(test))]
unsafe fn panic(_info: &core::panic::PanicInfo) -> ! {
    sys::platform_halt();
    loop {}
}

mod sys {
    extern "C" {
        pub fn platform_halt();
    }
}
