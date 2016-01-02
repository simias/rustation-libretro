pub mod libretro;
mod retrogl;
mod retrolog;

use std::path::Path;

use libc::{c_char, c_uint};

extern crate libc;
extern crate gl;
#[macro_use]
extern crate log;
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
    retrogl: retrogl::RetroGl,
}

impl Context {
    fn new() -> Option<Context> {
        retrogl::RetroGl::new()
            .map(|r|
                 Context {
                     retrogl: r,
                 })
    }
}

impl libretro::Context for Context {

    fn render_frame(&mut self) {
        match self.retrogl.state() {
            Some(s) => {
                if let Err(e) = s.render_frame() {
                    error!("Couldn't render frame: {:?}", e);
                }
            }
            None => {
                error!("Frame requested while we have no RetroGL state!");
                return;
            }
        }

        libretro::gl_frame_done(self.retrogl.xres(), self.retrogl.yres())
    }

    fn get_system_av_info(&self) -> libretro::SystemAvInfo {
        libretro::SystemAvInfo {
            geometry: libretro::GameGeometry {
                base_width: self.retrogl.xres() as c_uint,
                base_height: self.retrogl.yres() as c_uint,
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

    fn gl_context_reset(&mut self) {
        self.retrogl.context_reset();
    }

    fn gl_context_destroy(&mut self) {
        self.retrogl.context_destroy();
    }
}

/// Init function, called only once when our core gets loaded
fn init() {
    retrolog::init();
}

/// Called when a game is loaded and a new context must be built
fn load_game(game: &Path) -> Option<Box<libretro::Context>> {
    info!("Loading {:?}", game);

    Context::new()
        .map(|c| Box::new(c) as Box<libretro::Context>)
}
