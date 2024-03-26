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
                match parse_uki(file.clone()) {
                    Ok(config) => {
                        if let Some(splash) = config.splash {
                            let _u = show_splash(file.clone(), splash);
                        }
                        if let Err(err) = boot(file.clone(), config) {
                            println!("oof: {:?}", err)
                        }
                    }
                    Err(err) => println!("oof: {:?}", err),
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
        
        self.file.seek(SeekFrom::End(0)).map_err(|_| ())
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
    #[snafu(display("failed to parse object file"))]
    InvalidObject,
    #[snafu(display("kernel not found"))]
    KernelNotFound,
    #[snafu(display("initramfs not found"))]
    InitrdNotFound,
    #[snafu(display("DTB not found"))]
    DtbNotFound,
}

fn parse_uki(
    file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
) -> Result<BootConfig, UkiParseError> {
    let reader = ReadCache::new(FatFileReadCacheOps { file: file.clone() });
    let obj = File::parse(&reader).map_err(|_| UkiParseError::InvalidObject)?;

    let kernel = obj
        .section_by_name(".linux")
        .and_then(|v| v.file_range())
        .ok_or(UkiParseError::KernelNotFound)?;

    let initrd = obj
        .section_by_name(".initrd")
        .and_then(|v| v.file_range())
        .ok_or(UkiParseError::InitrdNotFound)?;

    // TODO: multiple dtbs
    // TODO: check picked DTB size
    let dtb = obj
        .section_by_name(".dtb")
        .and_then(|v| v.file_range())
        .ok_or(UkiParseError::DtbNotFound)?;

    let commandline = obj
        .section_by_name(".cmdline")
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

    fn boot_linux(
        kernel: *mut c_void,
        tags: *mut c_void,
        cmdline: *const c_char,
        machtype: c_uint,
        ramdisk: *mut c_void,
        ramdisk_size: c_uint,
        boot_type: c_uint,
    );

    fn board_machtype() -> c_uint;
}

#[derive(Debug, Snafu)]
enum BootError {
    #[snafu(display("I/O error"))]
    Io,
    #[snafu(display("loaded kernel had invalid magic"))]
    InvalidKernel,
    #[snafu(display("DTB exceeds maximum 2MB"))]
    DtbTooBig,
    Failed,
}

// Boot a kernel from provided BootConfig
// https://docs.kernel.org/arch/arm64/booting.html
// TODO: currently 64-bit only
fn boot(
    mut file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
    config: BootConfig,
) -> Result<(), BootError> {
    // DTB may not exceed 2mb.
    if config.dtb.1 > 2*1024*1024 {
        return Err(BootError::DtbTooBig);
    }

    let base = unsafe { get_ddr_start() } as u64;

    let (start, size) = config.kernel;
    // Read text_offset + image_size + magic from kernel header
    let mut buf = [0; 8];
    file.seek(SeekFrom::Start(start + 8)).map_err(|_| BootError::Io)?;
    file.read_exact(&mut buf).map_err(|_| BootError::Io)?;
    let text_offset = LittleEndian::read_u64(&buf);
    file.read_exact(&mut buf).map_err(|_| BootError::Io)?;
    let image_size = LittleEndian::read_u64(&buf);
    file.seek(SeekFrom::Current(32)).map_err(|_| BootError::Io)?;
    file.read_exact(&mut buf[0..4]).map_err(|_| BootError::Io)?;
    let magic = LittleEndian::read_u32(&buf);
    if magic != 0x644d5241 {
        return Err(BootError::InvalidKernel);
    }

    // Load kernel into base of DRAM.
    let kernel_addr = base + text_offset;
    let kernel = unsafe { &mut *slice_from_raw_parts_mut(kernel_addr as *mut u8, size as usize) };
    file.seek(SeekFrom::Start(start)).map_err(|_| BootError::Io)?;
    file.read_exact(kernel).map_err(|_| BootError::Io)?;

    // Load DTB after kernel image.
    let mut dtb_addr = kernel_addr + image_size;
    // DTB must be in its own 2MB region.
    dtb_addr += 2*1024*1024;
    // DTB address must be 8-byte aligned.
    dtb_addr += 8;
    dtb_addr &= !0b111;
    let (start, dtb_size) = config.dtb;
    let dtb = unsafe { &mut *slice_from_raw_parts_mut(dtb_addr as *mut u8, dtb_size as usize) };
    file.seek(SeekFrom::Start(start))
        .map_err(|_| BootError::Io)?;
    file.read_exact(dtb).map_err(|_| BootError::Io)?;

    // Load initrd.
    // Place initrd exactly 2mb after the start of DTB. This way we know that a) the initramfs is
    // not overlapping the special DTB 2mb region and b) is already aligned.
    let initrd_addr = dtb_addr + 2*1024*1024;
    let (start, initrd_size) = config.initrd;
    let initrd = unsafe { &mut *slice_from_raw_parts_mut(initrd_addr as *mut u8, initrd_size as usize) };
    file.seek(SeekFrom::Start(start))
        .map_err(|_| BootError::Io)?;
    file.read_exact(initrd).map_err(|_| BootError::Io)?;

    // Making sure it really is this code that booted the kernel ;)
    let mut wow = config.commandline.unwrap().to_string_lossy().to_string();
    wow += " haha cool";
    let wow = CString::new(wow).unwrap();

    // Do the boot!
    unsafe {
        boot_linux(
            base as *mut _,
            dtb_addr as *mut _,
            wow.as_c_str().as_ptr(),
            board_machtype(),
            initrd_addr as *mut _,
            initrd_size as c_uint,
            0,
        )
    }

    // If we got here then the boot failed.
    Err(BootError::Failed)
}

fn show_splash(
    mut file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
    (start, size): (u64, u64),
) -> Result<(), ()> {
    let mut splash = vec![0; size as usize];
    file.seek(SeekFrom::Start(start)).map_err(|_| ())?;
    file.read_exact(&mut splash).map_err(|_| ())?;

    let mut display = fbcon::get().ok_or(())?;
    let bmp = Bmp::<Rgb888>::from_slice(&splash).map_err(|_| ())?;
    let mut pos = Point::zero();
    pos.x = display.bounding_box().center().x - (bmp.size().width as i32) / 2;
    pos.y = display.bounding_box().bottom_right().unwrap().y - bmp.size().height as i32;
    let img = Image::new(&bmp, pos);
    img.draw(&mut display).map_err(|_| ())
}
