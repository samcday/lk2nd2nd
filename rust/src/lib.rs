#![no_std]

extern crate alloc;

use alloc::string::{ToString};
use alloc::{format, vec};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use anyhow::Error;

use byteorder::{ByteOrder};
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
    fn label(&self) -> &str;
    // fn splash<'a>() -> Option<Image<'a, Rgb888>>;
    fn boot(&mut self) -> !;
}

extern "C" {
    fn wait_key() -> u16;
}

const KEY_VOLUMEUP: u16 = 0x115;
const KEY_VOLUMEDOWN: u16 = 0x116;
const KEY_POWER: u16 = 0x119;

pub type FatFS = FileSystem<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>;

#[no_mangle]
pub extern "C" fn boot_scan() {
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
        print_menu(selected, &options, &mut display);

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
                        // if let Some(splash) = config.splash {
                        //     let _u = show_splash(file.clone(), splash);
                        // }
                    }
                    Err(err) => println!("oof: {:?}", err),
                }
            }
        }
    }

    Ok(())
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
