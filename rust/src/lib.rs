#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::string::{String, ToString};
use core::ffi::{c_char, c_uint, c_void};
use core::ptr::slice_from_raw_parts_mut;
use core::time::Duration;

use byteorder::{ByteOrder, LittleEndian};
use embedded_graphics::{
    mono_font::MonoTextStyle
    ,
    prelude::*,
    primitives::{
        Circle, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment, Triangle,
    },
    text::{Alignment, Text},
};
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_vintage_fonts::FONT_24X32;
use fatfs::{DefaultTimeProvider, Dir, IoError, LossyOemCpConverter, Read, Seek, SeekFrom};
use object::{File, Object, ObjectSection, ReadCache, ReadCacheOps};
use snafu::prelude::*;
use tinybmp::Bmp;

use crate::bio::OpenDevice;
use crate::lk_thread::sleep;

mod bio;
mod fmt;
mod lk_alloc;
mod lk_list;
mod panic;
mod lk_thread;
mod lk_mutex;
mod fbcon;

#[no_mangle]
pub extern "C" fn boot_scan() {
    lk_thread::spawn("boot-scan", || {
        let esp_dev = bio::get_bdevs().unwrap()
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
                } else if entry.file_name().ends_with(".efi") {
                    println!("parsing {} of size {}", name, entry.len());
                    let file = entry.to_file();
                    match parse_uki(file.clone()) {
                        Ok(config) => if let Err(err) = boot(file.clone(), config) {
                            println!("oof: {:?}", err)
                        }
                        Err(err) => println!("oof: {:?}", err),
                    }
                }
            }
        }
    }
}

struct FatFileReadCacheOps<'a> {
    file: fatfs::File<'a, OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
}

impl<'a> ReadCacheOps for FatFileReadCacheOps<'a> {
    fn len(&mut self) -> Result<u64, ()> {
        let res = self.file.seek(SeekFrom::End(0)).map_err(|_| ());
        res
    }

    fn seek(&mut self, pos: u64) -> Result<u64, ()> {
        self.file.seek(SeekFrom::Start(pos)).map_err(|_| ())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        self.file.read(buf).map_err(|_| ())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ()> {
        self.file.read_exact(buf).map_err(|_| ())
    }
}

#[derive(Debug)]
struct BootConfig {
    kernel: (u64, u64),
    initrd: (u64, u64),
    commandline: Option<CString>,
    dtb: (u64, u64),
    splash: Option<(u64, u64)>,
}

#[derive(Debug, Snafu)]
enum UkiParseError {
    #[snafu(display("failed to parse object file"))] InvalidObject,
    #[snafu(display("kernel not found"))] KernelNotFound,
    #[snafu(display("initramfs not found"))] InitrdNotFound,
    #[snafu(display("DTB not found"))] DtbNotFound,
}

fn parse_uki(file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>) -> Result<BootConfig, UkiParseError> {
    let reader = ReadCache::new(FatFileReadCacheOps { file: file.clone() });
    let obj = File::parse(&reader).map_err(|_| UkiParseError::InvalidObject)?;

    let kernel = obj.section_by_name(".linux")
        .and_then(|v| v.file_range()).ok_or(UkiParseError::KernelNotFound)?;

    let initrd = obj.section_by_name(".initrd")
        .and_then(|v| v.file_range()).ok_or(UkiParseError::InitrdNotFound)?;

    // TODO: multiple dtbs
    // TODO: check picked DTB size
    let dtb = obj.section_by_name(".dtb")
        .and_then(|v| v.file_range()).ok_or(UkiParseError::DtbNotFound)?;

    let commandline = obj.section_by_name(".cmdline")
        .and_then(|v| v.data().ok())
        .and_then(|v| CString::new(v).ok());

    let splash = obj.section_by_name(".splash").and_then(|v| v.file_range());

    Ok(BootConfig {
        kernel,
        initrd,
        dtb,
        commandline,
        splash,
    })
}

extern "C" {
    fn get_ddr_start() -> c_uint;

    fn boot_linux(kernel: *mut c_void, tags: *mut c_void,
                  cmdline: *const c_char, machtype: c_uint,
                  ramdisk: *mut c_void, ramdisk_size: c_uint,
                  boot_type: c_uint);

    fn board_machtype() -> c_uint;
}

#[derive(Debug, Snafu)]
enum BootError {
    #[snafu(display("I/O error"))]
    Io,
    #[snafu(display("loaded kernel had invalid magic"))]
    InvalidKernel,
}

pub fn boot(mut file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>, config: BootConfig) -> Result<(), BootError> {
    let base = unsafe { get_ddr_start() } as *mut u8;

    let align = 1024*1024*32;

    // TODO: respect text_offset
    let (start, size) = config.kernel;
    let kernel_size = size as usize;
    let kernel = unsafe { &mut *slice_from_raw_parts_mut(base, kernel_size) };
    file.seek(SeekFrom::Start(start)).map_err(|_| BootError::Io)?;
    file.read_exact(kernel).map_err(|_| BootError::Io)?;

    let magic = LittleEndian::read_u32(&kernel[56..60]);
    if magic != 0x644d5241 {
        return Err(BootError::InvalidKernel);
    }

    let dtb_addr = unsafe { base.add(kernel_size).add(kernel_size % align) };
    // let dtb_addr = 0x82000000 as *mut u8;
    let (start, size) = config.dtb;
    let dtb_size = size as usize;
    let dtb = unsafe { &mut *slice_from_raw_parts_mut(dtb_addr, dtb_size) };
    file.seek(SeekFrom::Start(start)).map_err(|_| BootError::Io)?;
    file.read_exact(dtb).map_err(|_| BootError::Io)?;

    let initrd_addr = unsafe { dtb_addr.add(dtb_size).add(dtb_size % 4096) };
    // let initrd_addr = 0x82200000 as *mut u8;
    let (start, initrd_size) = config.initrd;
    let initrd = unsafe { &mut *slice_from_raw_parts_mut(initrd_addr, initrd_size as usize) };
    file.seek(SeekFrom::Start(start)).map_err(|_| BootError::Io)?;
    file.read_exact(initrd).map_err(|_| BootError::Io)?;

    let mut wow = config.commandline.unwrap().to_string_lossy().to_string();
    wow = wow + " haha cool";
    let wow = CString::new(wow).unwrap();

    unsafe { boot_linux(base as *mut _, dtb_addr as *mut _, wow.as_c_str().as_ptr(), board_machtype(), initrd_addr as *mut _, initrd_size  as c_uint, 0) }

    Ok(())
}

fn show_splash() {
    // for section in obj.sections() {
    //     if section.name().is_ok_and(|sec| sec == ".splash") {
    //         match section.data() {
    //             Ok(data) => {
    //                 println!("loaded bmp data: {}", data.len());
    //                 let mut display = fbcon::get().unwrap();
    //                 match Bmp::<Rgb888>::from_slice(data) {
    //                     Ok(bmp) => {
    //                         let mut pos = Point::zero();
    //                         pos.x = display.bounding_box().center().x - (bmp.size().width as i32) / 2;
    //                         pos.y = display.bounding_box().bottom_right().unwrap().y - bmp.size().height as i32;
    //                         let img = Image::new(&bmp, pos);
    //                         let _ = img.draw(&mut display);
    //                     }
    //                     Err(e) => {
    //                         println!("noes :< {:?}", e);
    //                     }
    //                 }
    //             }
    //             Err(err) => {
    //                 println!("shiet: {}", err);
    //             }
    //         }
    //     }
    // }
}