#![no_std]

extern crate alloc;

use alloc::ffi::CString;
use alloc::string::ToString;
use alloc::vec;
use core::ffi::{c_char, c_uint, c_void};
use core::ptr::slice_from_raw_parts_mut;
use core::time::Duration;

use byteorder::{ByteOrder, LittleEndian};
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
use object::{File, Object, ObjectSection, ReadCache, ReadCacheOps};
use snafu::prelude::*;
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

#[no_mangle]
pub extern "C" fn boot_scan() {
    lk_thread::spawn("boot-scan", || {
        let esp_dev = bio::get_bdevs()
            .unwrap()
            .find(|dev| dev.label().is_some_and(|label| label.eq(c"esp")));

        if let Some(esp_dev) = esp_dev {
            println!("found ESP partition: {:?}", esp_dev.name());

            if let Some(dev) = bio::open(esp_dev.name()) {
                match fatfs::FileSystem::new(dev, fatfs::FsOptions::new()) {
                    Ok(fs) => {
                        let root_dir = fs.root_dir();
                        if let Ok(esp_dir) = root_dir.open_dir("/EFI/") {
                            scan_esp(esp_dir);
                        }
                    }
                    Err(e) => println!("noes! {:?}", e),
                }
            } else {
                println!("failed to open :<");
            }
        }
        lk_thread::exit()
    });

    let mut display = fbcon::get().unwrap();
    display.clear(Rgb888::CSS_BLACK).unwrap();

    let color = Rgb888::new(255, 0, 0);
    // Create styles used by the drawing operations.
    let thin_stroke = PrimitiveStyle::with_stroke(color, 1);
    let thick_stroke = PrimitiveStyle::with_stroke(color, 3);
    let border_stroke = PrimitiveStyleBuilder::new()
        .stroke_color(color)
        .stroke_width(3)
        .stroke_alignment(StrokeAlignment::Inside)
        .build();
    let fill = PrimitiveStyle::with_fill(color);
    let character_style = MonoTextStyle::new(&FONT_24X32, color);

    let text = Text::with_alignment(
        "Rust ftw",
        display.bounding_box().center() + Point::new(0, 15),
        character_style,
        Alignment::Center,
    );

    // Draw a 3px wide outline around the display.
    display
        .bounding_box()
        .into_styled(border_stroke)
        .draw(&mut display)
        .unwrap();

    let yoffset = 14;

    // Draw a triangle.
    Triangle::new(
        Point::new(16, 16 + yoffset),
        Point::new(16 + 16, 16 + yoffset),
        Point::new(16 + 8, yoffset),
    )
    .into_styled(thin_stroke)
    .draw(&mut display)
    .unwrap();

    // Draw a filled square
    Rectangle::new(Point::new(52, yoffset), Size::new(16, 16))
        .into_styled(fill)
        .draw(&mut display)
        .unwrap();

    // Draw a circle with a 3px wide stroke.
    Circle::new(Point::new(88, yoffset), 17)
        .into_styled(thick_stroke)
        .draw(&mut display)
        .unwrap();

    // Draw centered text
    display
        .fill_solid(&text.bounding_box(), Rgb888::CSS_BLACK)
        .unwrap();
    text.draw(&mut display).unwrap();

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
                        if let Err(err) = kernel_boot::boot(file.clone(), config) {
                            println!("oof: {:?}", err)
                        }
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
