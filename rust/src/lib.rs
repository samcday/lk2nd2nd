#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::{vec};
use alloc::boxed::Box;
use core::time::Duration;

use byteorder::{ByteOrder};
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{
        Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
    },
    text::{Alignment, Text},
};
use embedded_vintage_fonts::FONT_24X32;
use fatfs::{DefaultTimeProvider, Dir, LossyOemCpConverter, Read, Seek, SeekFrom};
use object::{Object, ObjectSection, ReadCacheOps};
use tinybmp::Bmp;

use crate::bio::OpenDevice;
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
    fn label() -> String;
    // fn splash<'a>() -> Option<Image<'a, Rgb888>>;
    // fn boot() -> !;
}

#[no_mangle]
pub extern "C" fn boot_scan() {
    // lk_thread::spawn("boot-scan", || {
        for dev in bio::get_bdevs().unwrap().iter().filter(|dev| dev.is_leaf) {
            // TODO: expose type GUID in bdev and use that to check for ESP instead.
            if let Some(esp_dev) = dev.label.clone().filter(|label| label.eq("esp")).and_then(|_| bio::open(&dev.name).ok()) {
                println!("found ESP partition: {:?}", dev.name);
                match fatfs::FileSystem::new(esp_dev, fatfs::FsOptions::new()) {
                    Ok(fs) => {
                        let root_dir = fs.root_dir();
                        if let Ok(esp_dir) = root_dir.open_dir("/EFI/") {
                            scan_esp(esp_dir);
                        }
                    }
                    Err(e) => println!("noes! {:?}", e),
                }
            }

            match extlinux::scan(&dev.name) {
                Ok(_) => { println!("worked {}", dev.name); },
                Err(err) => {} //println!("{} extlinux scan failed: {:?}", &dev.name, err)
            }
        }

        // TODO: check for magic in boot partition
    //     lk_thread::exit()
    // });

    let mut display = fbcon::get().unwrap();
    display.clear(Rgb888::CSS_BLACK).unwrap();

    loop {
        sleep(Duration::from_millis(100));
    }
}

fn scan_esp(dir: Dir<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>) {
    for entry in dir.iter().flatten() {
        let name = entry.file_name();
        if name != ".." && name != "." {
            if entry.is_dir() {
                scan_esp(entry.to_dir());
            } else if entry.file_name().ends_with(".efi") {
                println!("parsing {} of size {}", name, entry.len());
                let file = entry.to_file();
                match kernel_boot::parse_uki(file.clone()) {
                    Ok(config) => {
                        if let Some(splash) = config.splash {
                            let _u = show_splash(file.clone(), splash);
                        }
                        // if let Err(err) = kernel_boot::boot(file.clone(), config) {
                        //     println!("oof: {:?}", err)
                        // }
                    }
                    Err(err) => println!("oof: {:?}", err),
                }
            }
        }
    }
}

fn show_splash(
    mut file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
    (start, size): (u64, u64),
) -> Result<(), ()> {
    let mut splash = vec![0; size as usize];
    file.seek(SeekFrom::Start(start)).map_err(|_| ())?;
    file.read_exact(&mut splash).map_err(|_| ())?;

    let mut display = fbcon::get().ok_or(())?;
    let bmp = Bmp::<Rgb888>::from_slice(&splash).map_err(|_| ())?.with_alpha_bg(Rgb888::CSS_BLACK);
    let mut pos = Point::zero();
    pos.x = display.bounding_box().center().x - (bmp.size().width as i32) / 2;
    pos.y = display.bounding_box().bottom_right().unwrap().y - bmp.size().height as i32;
    let img = Image::new(&bmp, pos);
    img.draw(&mut display).map_err(|_| ())
}
