#![no_std]

mod bio;
mod fmt;
mod lk_alloc;
mod lk_list;

extern crate alloc;

use crate::lk_alloc::LkHeap;

use core::panic::PanicInfo;

#[global_allocator]
static ALLOCATOR: LkHeap = LkHeap;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn rust_hello_world() {
    let esp_dev = bio::get_bdevs()
        .into_iter()
        .find(|dev| dev.label().is_some_and(|label| label.eq(c"esp")));
    if let Some(esp_dev) = esp_dev {
        println!("found ESP partition: {:?}", esp_dev.name());
    }
}
