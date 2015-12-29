pub mod libretro;

use std::path::Path;

use libc::{c_char, c_uint};

extern crate libc;
extern crate glium;
extern crate rustation;

macro_rules! cstring {
    ($x:expr) => {
        concat!($x, '\0') as *const _ as *const c_char
    };
}

/// Static system information sent to the frontend on request
const SYSTEM_INFO: libretro::SystemInfo = libretro::SystemInfo {
    library_name: cstring!("Rustation"),
    library_version: rustation::VERSION_CSTR as *const _ as *const c_char,
    valid_extensions: cstring!("bin"),
    need_fullpath: false,
    block_extract: false,
};

/// Emulator context
struct Context {
    facade: Facade,
}

impl Context {
    fn new() -> Context {
        Context {
            facade: Facade::new(),
        }
    }
}

impl libretro::Context for Context {
    fn render_frame(&mut self) {
    }

    fn get_system_av_info(&self) -> libretro::SystemAvInfo {
        libretro::SystemAvInfo {
            geometry: libretro::GameGeometry {
                base_width: self.facade.xres() as c_uint,
                base_height: self.facade.yres() as c_uint,
                max_width: 640,
                max_height: 576,
                aspect_ratio: -1.0,
            },
            timing: libretro::SystemTiming {
                fps: 60.,
                sample_rate: 44_100.
            }
        }
    }
}

/// Called when a game is loaded and a new context must be built
fn load_game(_: &Path) -> Option<Box<libretro::Context>> {
    Some(Box::new(Context::new()) as Box<libretro::Context>)
}

/// Libretro facade for glium
struct Facade {
    xres: u16,
    yres: u16,
}

impl Facade {
    fn new() -> Facade {
        Facade {
            xres: 640,
            yres: 480,
        }
    }

    fn xres(&self) -> u16 {
        self.xres
    }

    fn yres(&self) -> u16 {
        self.yres
    }
}
