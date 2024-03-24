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

use core::time::Duration;
use crate::bio::OpenDevice;
use crate::fat_readcache::ReadCache;

use fatfs::{DefaultTimeProvider, Dir, File, LossyOemCpConverter};
use object::{Object, ObjectSection};
use crate::lk_thread::sleep;

#[no_mangle]
pub extern "C" fn rust_app() {
    lk_thread::spawn("test", || {
        let mut esp_dev = None;
        while esp_dev.is_none() {
            // lk2nd aboot app might not have initialized sdhci bdev yet. keep checking.
            // TODO: would be better to use lk event signalling for this.
            sleep(Duration::from_millis(50));
            esp_dev = bio::get_bdevs().unwrap()
                .find(|dev| dev.label().is_some_and(|label| label.eq(c"esp")));
        }

        if let Some(esp_dev) = esp_dev {
            println!("found ESP partition: {:?}", esp_dev.name());

            if let Some(dev) = bio::open(esp_dev.name()) {
                let fs = fatfs::FileSystem::new(dev, fatfs::FsOptions::new());
                if let Ok(fs) = fs {
                    let root_dir = fs.root_dir();
                    scan_esp(root_dir);
                }
            }
        }
         loop {
             sleep(Duration::from_secs(2));
             println!("still alive!");
         }
    });
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
            println!("section: {:?}", section.name());
        }
    }
}
