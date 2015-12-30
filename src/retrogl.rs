//! PlayStation OpenGL 3.3 renderer playing nice with libretro

use libretro;
use gl;

pub struct RetroGl {
    /// Current horizontal resolution of the video output
    xres: u16,
    /// Current vertical resolution of the video output
    yres: u16,
}

impl RetroGl {
    pub fn new() -> Option<RetroGl> {
        if !libretro::hw_context::init() {
            println!("Failed to init hardware context");
            return None;
        }

        gl::load_with(|s| {
            libretro::hw_context::get_proc_address(s) as *const _
        });

        Some(RetroGl {
            xres: 640,
            yres: 480,
        })
    }

    pub fn xres(&self) -> u16 {
        self.xres
    }

    pub fn yres(&self) -> u16 {
        self.yres
    }
}
