#![no_std]

extern crate alloc;

mod bio;
mod fat_readcache;
mod fmt;
mod lk_alloc;
mod lk_list;
mod panic;
mod lk_thread;
mod lk_mutex;
mod fbcon;

use core::time::Duration;
use crate::bio::{LkBlockDev, OpenDevice};
use crate::fat_readcache::ReadCache;

use fatfs::{DefaultTimeProvider, Dir, File, LossyOemCpConverter};
use object::{Object, ObjectSection};
use crate::fbcon::fbcon_display;
use crate::lk_thread::sleep;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{
        Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
    },
    text::{Alignment, Text},
};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::text::TextStyle;
use embedded_vintage_fonts::FONT_24X32;

#[no_mangle]
pub extern "C" fn boot_scan() {
    lk_thread::spawn("boot-scan", || {
        let mut esp_dev = bio::get_bdevs().unwrap()
            .find(|dev| dev.label().is_some_and(|label| label.eq(c"esp")));

        if let Some(esp_dev) = esp_dev {
            println!("found ESP partition: {:?}", esp_dev.name());

            if let Some(dev) = bio::open(esp_dev.name()) {
                match fatfs::FileSystem::new(dev, fatfs::FsOptions::new()) {
                    Ok(fs) => {
                        let root_dir = fs.root_dir();
                        scan_esp(root_dir);
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

    let mut offset = 0;
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

    let mut text = Text::with_alignment(
        "Rust ftw",
        display.bounding_box().center() + Point::new(offset, 15),
        character_style,
        Alignment::Center,
    );

    // Draw a 3px wide outline around the display.
    display
        .bounding_box()
        .into_styled(border_stroke)
        .draw(&mut display).unwrap();

    let yoffset = 14;

    // Draw a triangle.
    Triangle::new(
        Point::new(16, 16 + yoffset),
        Point::new(16 + 16, 16 + yoffset),
        Point::new(16 + 8, yoffset),
    )
        .into_styled(thin_stroke)
        .draw(&mut display).unwrap();

    // Draw a filled square
    Rectangle::new(Point::new(52, yoffset), Size::new(16, 16))
        .into_styled(fill)
        .draw(&mut display).unwrap();

    // Draw a circle with a 3px wide stroke.
    Circle::new(Point::new(88, yoffset), 17)
        .into_styled(thick_stroke)
        .draw(&mut display).unwrap();

    // Draw centered text
    display.fill_solid(&text.bounding_box(), Rgb888::CSS_BLACK).unwrap();
    text.draw(&mut display).unwrap();

    loop {
        sleep(Duration::from_millis(100));
    }
}

fn scan_esp(dir: Dir<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>) {
    for entry in dir.iter() {
        if let Ok(entry) = entry {
            let name = entry.file_name();
            if name != ".." && name != "." {
                if entry.is_dir() {
                    scan_esp(entry.to_dir());
                } else {
                    println!("parsing {} of size {}", name, entry.len());
                    parse_esp_file(entry.to_file(), entry.len());
                }
            }
        }
    }
}

fn parse_esp_file(file: File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>, _size: u64) {
    let reader = ReadCache::new(file);
    if let Ok(obj) = object::File::parse(&reader) {
        for section in obj.sections() {
            if section.name().is_ok_and(|sec| sec == ".cmdline") {
                println!("cmdline is: {:?}", core::str::from_utf8(section.data().unwrap()).unwrap());
            }
        }
    }
}
