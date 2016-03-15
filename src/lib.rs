#[macro_use]
pub mod libretro;
#[macro_use]
mod retrogl;
mod retrolog;
mod renderer;

use std::str::FromStr;
use std::ptr;

use libc::{c_char, c_uint, int16_t, uint16_t, uint32_t};

use retrogl::RetroGl;
use renderer::CommandVertex;

#[macro_use]
extern crate log;
extern crate libc;
extern crate gl;
extern crate arrayvec;

static mut static_renderer: *mut retrogl::RetroGl = 0 as *mut _;

fn drop_renderer() {
    unsafe {
        if !static_renderer.is_null() {
            let _ = Box::from_raw(static_renderer);
            static_renderer = ptr::null_mut();
        }
    }
}

fn set_renderer(renderer: RetroGl) {
    let r = Box::new(renderer);

    drop_renderer();

    unsafe {
        static_renderer = Box::into_raw(r);
    }
}

fn renderer() -> &'static mut RetroGl {
    unsafe {
        if static_renderer.is_null() {
            panic!("Attempted to use a NULL renderer");
        }

        &mut *static_renderer
    }
}

#[no_mangle]
pub extern "C" fn rsx_init() {
    static mut first_init: bool = true;

    unsafe {
        if first_init {
            retrolog::init();
            first_init = false;
        }
    }
}

#[no_mangle]
pub extern "C" fn rsx_open(is_pal: bool) -> bool {
    let clock = match is_pal {
        true => VideoClock::Pal,
        false => VideoClock::Ntsc,
    };

    match RetroGl::new(clock) {
        Ok(r) => {
            set_renderer(r);
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn rsx_close() {
    drop_renderer();
}

#[no_mangle]
pub extern "C" fn rsx_refresh_variables() {
    renderer().refresh_variables();
}

#[no_mangle]
pub extern "C" fn rsx_prepare_frame() {
    renderer().prepare_render();
}

#[no_mangle]
pub extern "C" fn rsx_finalize_frame() {
    renderer().finalize_frame();
}

#[no_mangle]
pub extern "C" fn rsx_set_environment(callback: libretro::EnvironmentFn) {
    libretro::set_environment(callback);
}

#[no_mangle]
pub extern "C" fn rsx_set_video_refresh(callback: libretro::VideoRefreshFn) {
    libretro::set_video_refresh(callback);
}

#[no_mangle]
pub extern "C" fn rsx_get_system_av_info(info: *mut libretro::SystemAvInfo) {
    let info = ptr_as_mut_ref(info).unwrap();

    *info = renderer().get_system_av_info();
}

//
// Draw commands
//

#[no_mangle]
pub extern "C" fn rsx_set_draw_offset(x: int16_t, y: int16_t) {
    renderer().gl_renderer().set_draw_offset(x as i16, y as i16);
}

#[no_mangle]
pub extern "C" fn rsx_set_draw_area(x: uint16_t,
                                    y: uint16_t,
                                    w: uint16_t,
                                    h: uint16_t) {
    renderer().gl_renderer().set_draw_area((x as u16, y as u16),
                                           (w as u16, h as u16));
}

#[no_mangle]
pub extern "C" fn rsx_set_display_mode(x: uint16_t,
                                       y: uint16_t,
                                       w: uint16_t,
                                       h: uint16_t,
                                       depth_24bpp: bool) {
    renderer().gl_renderer().set_display_mode((x as u16, y as u16),
                                              (w as u16, h as u16),
                                              depth_24bpp);
}

#[no_mangle]
pub extern "C" fn rsx_push_triangle(p0x: int16_t,
                                    p0y: int16_t,
                                    p1x: int16_t,
                                    p1y: int16_t,
                                    p2x: int16_t,
                                    p2y: int16_t,
                                    c0: uint32_t,
                                    c1: uint32_t,
                                    c2: uint32_t,
                                    dither: bool) {

    let v = [
        CommandVertex {
            position: [p0x as i16, p0y as i16],
            color: [c0 as u8, (c0 >> 8) as u8, (c0 >> 16) as u8],
            texture_coord: [0; 2],
            texture_page: [0; 2],
            clut: [0; 2],
            texture_blend_mode: 0,
            depth_shift: 0,
            dither: dither as u8,
        },
        CommandVertex {
            position: [p1x as i16, p1y as i16],
            color: [c1 as u8, (c1 >> 8) as u8, (c1 >> 16) as u8],
            texture_coord: [0; 2],
            texture_page: [0; 2],
            clut: [0; 2],
            texture_blend_mode: 0,
            depth_shift: 0,
            dither: dither as u8,
        },
        CommandVertex {
            position: [p2x as i16, p2y as i16],
            color: [c2 as u8, (c2 >> 8) as u8, (c2 >> 16) as u8],
            texture_coord: [0; 2],
            texture_page: [0; 2],
            clut: [0; 2],
            texture_blend_mode: 0,
            depth_shift: 0,
            dither: dither as u8,
        }];

    renderer().gl_renderer().push_triangle(&v);
}

#[no_mangle]
pub extern "C" fn rsx_load_image(x: uint16_t, y: uint16_t,
                                 w: uint16_t, h: uint16_t,
                                 vram: *const uint16_t) {
    let vram = unsafe {
        ::std::slice::from_raw_parts(vram as *const u16, 1024 * 512)
    };

    renderer().gl_renderer().upload_vram_window((x as u16, y as u16),
                                                (w as u16, h as u16),
                                                vram).unwrap();
}

/// Cast a mutable pointer into a mutable reference, return None if
/// it's NULL.
fn ptr_as_mut_ref<'a, T>(v: *mut T) -> Option<&'a mut T> {

    if v.is_null() {
        None
    } else {
        Some(unsafe { &mut *v })
    }
}

libretro_variables!(
    struct CoreVariables (prefix = "beetle_psx") {
        internal_resolution: u32, parse_upscale
            => "Internal upscaling factor; \
                1x (native)|2x|3x|4x|5x|6x|7x|8x|9x|10x|11x|12x",
        internal_color_depth: u8, parse_color_depth
            => "Internal color depth; dithered 16bpp (native)|32bpp",
        scale_dither: bool, parse_bool
            => "Scale dithering pattern with internal resolution; \
                enabled|disabled",
        wireframe: bool, parse_bool
            => "Wireframe mode; disabled|enabled",
        bios_menu: bool, parse_bool
            => "Boot to BIOS menu; disabled|enabled",
        display_internal_fps: bool, parse_bool
            => "Display internal FPS; disabled|enabled"
    });

fn parse_upscale(opt: &str) -> Result<u32, <u32 as FromStr>::Err> {
    let num = opt.trim_matches(|c: char| !c.is_numeric());

    num.parse()
}

fn parse_color_depth(opt: &str) -> Result<u8, <u8 as FromStr>::Err> {
    let num = opt.trim_matches(|c: char| !c.is_numeric());

    num.parse()
}

fn parse_bool(opt: &str) -> Result<bool, ()> {
    match opt {
        "true" | "enabled" | "on" => Ok(true),
        "false" | "disabled" | "off" => Ok(false),
        _ => Err(()),
    }
}

// Precise FPS values for the video output for the given
// VideoClock. It's actually possible to configure the PlayStation GPU
// to output with NTSC timings with the PAL clock (and vice-versa)
// which would make this code invalid but it wouldn't make a lot of
// sense for a game to do that.
fn video_output_framerate(std: VideoClock) -> f32 {
    match std {
        // 53.690MHz GPU clock frequency, 263 lines per field,
        // 3413 cycles per line
        VideoClock::Ntsc => 59.81,
        // 53.222MHz GPU clock frequency, 314 lines per field,
        // 3406 cycles per line
        VideoClock::Pal => 49.76,
    }
}

fn get_av_info(std: VideoClock, upscaling: u32) -> libretro::SystemAvInfo {

    // Maximum resolution supported by the PlayStation video
    // output is 640x480
    let max_width = (640 * upscaling) as c_uint;
    let max_height = (480 * upscaling) as c_uint;

    libretro::SystemAvInfo {
        geometry: libretro::GameGeometry {
            // The base resolution will be overriden using
            // ENVIRONMENT_SET_GEOMETRY before rendering a frame so
            // this base value is not really important
            base_width: max_width,
            base_height: max_height,
            max_width: max_width,
            max_height: max_height,
            aspect_ratio: 4./3.,
        },
        timing: libretro::SystemTiming {
            fps: video_output_framerate(std) as f64,
            sample_rate: 44_100.
        }
    }
}

// Width of the VRAM in 16bit pixels
pub const VRAM_WIDTH_PIXELS: u16 = 1024;
// Height of the VRAM in lines
pub const VRAM_HEIGHT: u16 = 512;

/// The are a few hardware differences between PAL and NTSC consoles,
/// in particular the pixelclock runs slightly slower on PAL consoles.
#[derive(Clone,Copy)]
pub enum VideoClock {
    Ntsc,
    Pal,
}
