#![no_std]

extern crate alloc;

use alloc::string::{ToString};
use alloc::{format, vec};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_uchar, c_void};
use core::ops::AddAssign;
use core::slice;
use core::time::Duration;
use anyhow::Error;

use byteorder::{ByteOrder};
use core2::io::{ErrorKind, Write};
use core2::io::ErrorKind::Other;
use embedded_graphics::image::Image;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_layout::layout::linear::{FixedMargin, LinearLayout};
use embedded_layout::prelude::*;
use fatfs::{DefaultTimeProvider, FileSystem, LossyOemCpConverter, Read, Seek, SeekFrom};
use object::{Object, ObjectSection, ReadCacheOps};
use profont::PROFONT_24_POINT;
use tinybmp::Bmp;
use txmodems::variants::xmodem::XModem;
use txmodems::common::{BlockLengthKind, ChecksumKind, ModemTrait, XModemTrait};

use crate::bio::OpenDevice;
use crate::fbcon::FbCon888;
use crate::fmt::_dputc;
use crate::lk_thread::sleep;

mod bio;
mod fbcon;
mod fmt;
mod lk_alloc;
mod lk_list;
mod lk_mutex;
mod lk_thread;
mod panic;
mod kernel_boot;
mod lk_fs;
mod extlinux;

trait BootOption {
    fn label(&self) -> &str;
    fn splash(&self, display: &mut FbCon888) -> Result<(), ()>;
    fn boot(&mut self) -> !;
}

extern "C" {
    fn wait_key() -> u16;

    fn uart_getc(port: c_int, wait: bool) -> c_int;
}

const KEY_VOLUMEUP: u16 = 0x115;
const KEY_VOLUMEDOWN: u16 = 0x116;
const KEY_POWER: u16 = 0x119;

pub type FatFS = FileSystem<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>;

struct SerialDevice {
}

impl core2::io::Read for SerialDevice {
    fn read(&mut self, mut buf: &mut [u8]) -> core2::io::Result<usize> {
        let mut inp: c_int = 0;
        for i in 0..buf.len() {
            inp = unsafe { uart_getc(0, true) };
            if inp < 0 {
                return Err(core2::io::Error::new(ErrorKind::Other, "err"));
            }
            buf[i] = inp as u8;
        }
        Ok(buf.len())
    }
}

impl core2::io::Write for SerialDevice {
    fn write(&mut self, buf: &[u8]) -> core2::io::Result<usize> {
        for b in buf {
            unsafe { _dputc(*b as c_char); }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> core2::io::Result<()> {
        Ok(())
    }
}

struct KernelDirectLoader {
    ptr: *mut u8,
}

impl core2::io::Write for KernelDirectLoader {
    fn write(&mut self, buf: &[u8]) -> core2::io::Result<usize> {
        unsafe { slice::from_raw_parts_mut(self.ptr, buf.len()).copy_from_slice(buf); }
        self.ptr = unsafe { self.ptr.add(buf.len()) };
        Ok(buf.len())
    }

    fn flush(&mut self) -> core2::io::Result<()> {
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn boot_scan() {
    // lk_thread::spawn("uart", || {
    //     let mut inp: c_char = 0;
    //
    //     loop {
    //         if unsafe { dgetc(&mut inp, true) } == 0 {
    //             println!("you said: {}", inp);
    //         }
    //     }
    // });

    lk_thread::spawn("xmodem", || {
        let mut modem = XModem::new();
        modem.block_length = BlockLengthKind::OneK;
        let mut dev = SerialDevice{};
        loop {
            println!("waiting...");
            loop {
                if unsafe { uart_getc(0, true) == 0x20 } {
                    break;
                }
            }
            let mut foo = KernelDirectLoader{ptr: unsafe { kernel_boot::sys::get_ddr_start() as *mut u8 } };
            println!("*Orc peon voice* ready to work...");
            sleep(Duration::from_millis(250));
            let result = modem.receive(&mut dev, &mut foo, ChecksumKind::Crc16);
            sleep(Duration::from_millis(250));
            match result {
                Ok(_) => {
                    println!("ok here we go I guess.");
                    // quintessential unsafe. Really, I don't think unsafe gets any more unsafe than this.
                    unsafe {
                        kernel_boot::sys::boot_linux(
                            kernel_boot::sys::get_ddr_start() as *mut c_void,
                            0 as *mut c_void,
                            0 as *mut c_char,
                            kernel_boot::sys::board_machtype(),
                            0 as *mut c_void,
                            0,
                            0
                        );
                    }
                },
                Err(e) => println!("onoes! {:?}", e),
            }
        }
        lk_thread::exit();
    });
    // lk_thread::spawn("boot-scan", || {
    let mut options: Vec<Box<dyn BootOption>> = Vec::new();

    for dev in bio::get_bdevs().unwrap().iter().filter(|dev| dev.is_leaf) {
        // TODO: expose type GUID in bdev and use that to check for ESP instead.
        if let Some(esp_dev) = dev.label.clone().filter(|label| label.eq("esp")).and_then(|_| bio::open(&dev.name).ok()) {
            println!("found ESP partition: {:?}", dev.name);
            match FatFS::new(esp_dev, fatfs::FsOptions::new()) {
                Ok(fs) => {
                    scan_esp(Arc::new(fs), "/EFI", &mut options);
                }
                Err(e) => println!("noes! {:?}", e),
            }
        }

        if let Ok(opt) = extlinux::scan(&dev.name) {
            options.push(opt);
        }
    }

        // TODO: check for magic in boot partition
    //     lk_thread::exit()
    // });

    let mut display = fbcon::get().unwrap();
    display.clear(Rgb888::CSS_BLACK).unwrap();

    let mut selected = 0;

    loop {
        display.clear(Rgb888::CSS_BLACK).unwrap();
        print_menu(selected, &options, &mut display);

        options[selected].splash(&mut display);

        match unsafe { wait_key() } {
            KEY_POWER => {
                options[selected].boot();
            }
            KEY_VOLUMEUP => {
                if selected == 0 {
                    selected = options.len();
                }
                selected -= 1;
            }
            KEY_VOLUMEDOWN => {
                selected += 1;
                if selected == options.len() {
                    selected = 0;
                }
            }
            _ => {}
        }
    }
}

fn print_menu<DT: DrawTarget<Color = Rgb888>>(selected: usize, options: &Vec<Box<dyn BootOption>>, display: &mut DT) {
    let text_style = MonoTextStyle::new(&PROFONT_24_POINT, Rgb888::CSS_SLATE_GRAY);
    let selected_text_style = MonoTextStyle::new(&PROFONT_24_POINT, Rgb888::CSS_HOT_PINK);

    let mut views: Vec<_> = options.iter().enumerate()
        .map(|(idx, option)| Text::new(option.label(), Point::zero(), if idx == selected { selected_text_style } else { text_style }))
        .collect();

    // let layout = LinearLayout::vertical(Chain::new(Text::new("Select a boot option", Point::zero(), text_style)).append(Views::new(&mut views)));
    let layout = LinearLayout::vertical(Views::new(&mut views));
    layout.with_alignment(horizontal::Center)
        .with_spacing(FixedMargin(10))
        .arrange()
        .align_to(&display.bounding_box(), horizontal::Center, vertical::Center)
        .draw(display);

}

fn scan_esp(fs: Arc<FatFS>, root: &str, options: &mut Vec<Box<dyn BootOption>>) -> anyhow::Result<()> {
    let dir = fs.root_dir().open_dir(root).map_err(Error::msg)?;
    for entry in dir.iter().flatten() {
        let name = entry.file_name();
        if name != ".." && name != "." {
            if entry.is_dir() {
                scan_esp(fs.clone(), &format!("{}/{}", root, entry.file_name()), options)?;
            } else if name.ends_with(".efi") {
                println!("parsing {} of size {}", name, entry.len());
                match kernel_boot::parse_uki(fs.clone(), &format!("{}/{}", root, name)) {
                    Ok(config) => {
                        options.push(Box::new(config));
                    }
                    Err(err) => println!("oof: {:?}", err),
                }
            }
        }
    }

    Ok(())
}
