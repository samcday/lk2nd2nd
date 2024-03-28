use alloc::ffi::CString;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::ffi::{c_char, c_uint, c_void};
use core::ptr::slice_from_raw_parts_mut;
use byteorder::{ByteOrder, LittleEndian};
use fatfs::{DefaultTimeProvider, LossyOemCpConverter, Read, Seek, SeekFrom};
use object::{File, Object, ObjectSection, ReadCache, ReadCacheOps};
use snafu::Snafu;
use crate::bio::OpenDevice;
use crate::{BootOption, FatFS, kernel_boot, println};

pub struct UkiBootConfig {
    fs: Arc<FatFS>,
    path: String,
    name: String,
    kernel: (u64, u64),
    initrd: (u64, u64),
    commandline: Option<CString>,
    dtb: (u64, u64),
    pub splash: Option<(u64, u64)>,
}

impl BootOption for UkiBootConfig {
    fn label(&self) -> &str {
        &self.name
    }

    fn boot(&mut self) -> ! {
        let file = self.fs.root_dir().open_file(&self.path).unwrap();
        if let Err(err) = boot(file, &self) {
            println!("oof: {:?}", err)
        }
        panic!("noes");
    }
}

#[derive(Debug, Snafu)]
pub enum UkiParseError {
    FileNotFound,
    #[snafu(display("failed to parse object file"))]
    InvalidObject,
    OSRelMissing,
    #[snafu(display("kernel not found"))]
    KernelNotFound,
    #[snafu(display("initramfs not found"))]
    InitrdNotFound,
    #[snafu(display("DTB not found"))]
    DtbNotFound,
}

pub fn parse_uki(fs: Arc<FatFS>, path: &str) -> Result<UkiBootConfig, UkiParseError> {
    let dir = fs.root_dir();
    let file = dir.open_file(path).map_err(|_| UkiParseError::FileNotFound)?;
    let reader = ReadCache::new(FatFileReadCacheOps { file: file.clone() });
    let obj = File::parse(&reader).map_err(|_| UkiParseError::InvalidObject)?;

    let name = obj.section_by_name(".osrel")
        .and_then(|v| v.data().ok())
        .and_then(|v| CString::new(v).ok())
        .and_then(|v| v.into_string().ok())
        .and_then(|v| v.split('\n')
            .find(|v| v.starts_with("PRETTY_NAME="))
            .and_then(|v| v.split("=").skip(1).next())
            .filter(|v| v.len() > 2)
            .and_then(|v| Some(String::from(&v[1..v.len()-1]))));
    if name.is_none() {
        return Err(UkiParseError::OSRelMissing);
    }

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

    Ok(UkiBootConfig {
        fs: fs.clone(),
        path: String::from(path),
        name: name.unwrap(),
        kernel,
        initrd,
        dtb,
        commandline,
        splash,
    })
}


#[derive(Debug, Snafu)]
pub enum BootError {
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
pub fn boot(
    mut file: fatfs::File<OpenDevice, DefaultTimeProvider, LossyOemCpConverter>,
    config: &UkiBootConfig,
) -> Result<(), BootError> {
    // DTB may not exceed 2mb.
    if config.dtb.1 > 2*1024*1024 {
        return Err(BootError::DtbTooBig);
    }

    let base = unsafe { sys::get_ddr_start() } as u64;

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
    let mut wow = config.commandline.clone().unwrap().to_string_lossy().to_string();
    wow += " haha cool";
    let wow = CString::new(wow).unwrap();

    // Do the boot!
    unsafe {
        sys::boot_linux(
            base as *mut _,
            dtb_addr as *mut _,
            wow.as_c_str().as_ptr(),
            sys::board_machtype(),
            initrd_addr as *mut _,
            initrd_size as c_uint,
            0,
        )
    }

    // If we got here then the boot failed.
    Err(BootError::Failed)
}

/// Trait glue to allow the object crate to read from a fatfs file.
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

mod sys {
    use core::ffi::{c_char, c_uint, c_void};

    extern "C" {
        pub fn get_ddr_start() -> c_uint;

        pub fn boot_linux(
            kernel: *mut c_void,
            tags: *mut c_void,
            cmdline: *const c_char,
            machtype: c_uint,
            ramdisk: *mut c_void,
            ramdisk_size: c_uint,
            boot_type: c_uint,
        );

        pub fn board_machtype() -> c_uint;
    }
}