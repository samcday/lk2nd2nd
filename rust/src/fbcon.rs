use core::convert::Infallible;
use core::ffi::{c_uint, c_void};
use core::ptr::slice_from_raw_parts_mut;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Dimensions, Size};
use embedded_graphics::Pixel;
use embedded_graphics::pixelcolor::{IntoStorage, Rgb565, Rgb888, RgbColor};
use embedded_graphics::prelude::Point;
use embedded_graphics::primitives::Rectangle;

pub struct FbCon888<'a> {
    width: u32,
    height: u32,
    stride: usize,
    buf: &'a mut [u8],
}

impl <'a> Dimensions for FbCon888<'a> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(self.width, self.height))
    }
}

impl <'a> DrawTarget for FbCon888<'a> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error> where I: IntoIterator<Item=Pixel<Self::Color>> {
        for pixel in pixels {
            let c = pixel.1;
            let p = pixel.0;
            let pos = (p.y as usize * self.stride * 3) + (p.x as usize * 3);
            self.buf[pos] = c.b();
            self.buf[pos + 1] = c.g();
            self.buf[pos + 2] = c.r();
        }
        Ok(())
    }
}

pub fn get<'a>() -> Option<FbCon888<'a>> {
    let fbcon = unsafe { fbcon_display() };
    if fbcon.is_null() {
        return None;
    }
    let fbcon = unsafe { &*fbcon };
    if fbcon.format != 3 {
        return None;
    }
    unsafe {
        Some(FbCon888 {
            width: fbcon.width,
            height: fbcon.height,
            stride: fbcon.stride as usize,
            buf: &mut *slice_from_raw_parts_mut(fbcon.buf.cast(), (fbcon.stride * fbcon.height * 3) as usize)
        })
    }
}

#[derive(Debug)]
#[repr(C)]
struct fbcon_config {
    buf: *mut c_void,
    width: c_uint,
    height: c_uint,
    stride: c_uint,
    bpp: c_uint,
    format: c_uint,
}

extern "C" {
    pub fn fbcon_display() -> *mut fbcon_config;
}
