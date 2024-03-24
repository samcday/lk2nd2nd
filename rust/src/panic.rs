extern "C" {
    pub fn platform_halt();
}

#[panic_handler]
#[cfg(not(test))]
unsafe fn panic(_info: &core::panic::PanicInfo) -> ! {
    platform_halt();
    loop {}
}
