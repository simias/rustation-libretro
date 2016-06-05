#[macro_use]
pub mod libretro;
#[macro_use]
mod retrogl;
mod retrolog;
mod renderer;
mod savestate;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use libc::{c_char, c_uint};

use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use rustation::cdrom::disc::{Disc, Region};
use rustation::bios::{Bios, BIOS_SIZE};
use rustation::gpu::{Gpu, VideoClock};
use rustation::memory::Interconnect;
use rustation::cpu::Cpu;
use rustation::padmemcard::gamepad::{Button, ButtonState};
use rustation::shared::SharedState;
use rustation::debugger::Debugger;

use cdimage::cue::Cue;

#[macro_use]
extern crate log;
extern crate libc;
extern crate gl;
extern crate rustation;
extern crate arrayvec;
extern crate cdimage;
extern crate rustc_serialize;

/// Static system information sent to the frontend on request
const SYSTEM_INFO: libretro::SystemInfo = libretro::SystemInfo {
    library_name: cstring!("Rustation"),
    library_version: rustation::VERSION_CSTR as *const _ as *const c_char,
    valid_extensions: cstring!("cue"),
    need_fullpath: false,
    block_extract: false,
};

/// Emulator context
struct Context {
    retrogl: retrogl::RetroGl,
    cpu: Cpu,
    shared_state: SharedState,
    debugger: Debugger,
    disc_path: PathBuf,
    video_clock: VideoClock,
    /// Number of frames output by the emulator (i.e. number of times
    /// `render_frame` has been called)
    frame_count: u32,
    /// When true the internal FPS monitoring in enabled
    monitor_internal_fps: bool,
    /// Number of frames we guessed the game rendered internally when
    /// `monitor_internal_fps` is true. This counter is reset every
    /// `INTERNAL_FPS_SAMPLE_PERIOD`.
    internal_frame_count: u32,
    /// Internal display coordinates in VRAM at the end of the
    /// previous frame. Used for internal FPS calculations.
    prev_display_start: (u16, u16),
    /// Cached value for the maximum savestate size in bytes
    savestate_max_len: usize,
}

impl Context {
    fn new(disc: &Path) -> Result<Context, ()> {

        let (cpu, video_clock) = try!(Context::load_disc(disc));
        let shared_state = SharedState::new();
        let retrogl = try!(retrogl::RetroGl::new(video_clock));

        let mut context =
            Context {
                retrogl: retrogl,
                cpu: cpu,
                shared_state: shared_state,
                debugger: Debugger::new(),
                disc_path: disc.to_path_buf(),
                video_clock: video_clock,
                frame_count: 0,
                monitor_internal_fps: CoreVariables::display_internal_fps(),
                internal_frame_count: 0,
                prev_display_start: (0, 0),
                savestate_max_len: 0,
            };

        let max_len = try!(context.compute_savestate_max_length());

        context.savestate_max_len = max_len;

        Ok(context)
    }

    fn compute_savestate_max_length(&mut self) -> Result<usize, ()> {
        // In order to get the full size we're just going to use a
        // dummy Write struct which will just count how many bytes are
        // being written
        struct WriteCounter(usize);

        impl ::std::io::Write for WriteCounter {
            fn write(&mut self, buf: &[u8]) -> ::std::io::Result<usize> {
                let len = buf.len();

                self.0 += len;

                Ok(len)
            }

            fn flush(&mut self) -> ::std::io::Result<()> {
                Ok(())
            }
        }

        let mut counter = WriteCounter(0);

        try!(self.save_state(&mut counter));

        let len = counter.0;

        // Our savestate format has variable length, in particular we
        // have the GPU's load_buffer which can grow to 1MB in the
        // worst case scenario (the entire VRAM). I'm going to be
        // optimistic here and give us 512KB of "headroom", that
        // should be enough 99% of the time, hopefully.
        let len = len + 512 * 1024;

        Ok(len)
    }

    fn save_state(&self, writer: &mut ::std::io::Write) -> Result<(), ()> {

        let mut encoder =
            match savestate::Encoder::new(writer) {
                Ok(encoder) => encoder,
                Err(e) => {
                    warn!("Couldn't create savestate encoder: {:?}", e);
                    return Err(())
                }
            };

        match self.encode(&mut encoder) {
            Ok(_) => Ok(()),
            Err(e) => {
                warn!("Couldn't serialize emulator state: {:?}", e);
                Err(())
            }
        }
    }

    fn load_state(&mut self, reader: &mut ::std::io::Read) -> Result<(), ()> {
        let mut decoder =
            match savestate::Decoder::new(reader) {
                Ok(decoder) => decoder,
                Err(e) => {
                    warn!("Couldn't create savestate decoder: {:?}", e);
                    return Err(())
                }
            };

        // I don't implement Decodable for Context itself because I
        // don't want to create a brand new instance. Things like the
        // debugger or disc path don't need to be reset

        let decoded =
            decoder.read_struct("Context", 4, |d| {
                let cpu = try!(d.read_struct_field("cpu", 0,
                                                   Decodable::decode));

                let retrogl = try!(d.read_struct_field("retrogl", 1,
                                                       Decodable::decode));

                let video_clock = try!(d.read_struct_field("video_clock", 2,
                                                           Decodable::decode));

                let shared_state = try!(d.read_struct_field("shared_state", 3,
                                                            Decodable::decode));

                Ok((cpu, retrogl, video_clock, shared_state))
            });

        let (cpu, retrogl, video_clock, shared_state) =
            match decoded {
                Ok(d) => d,
                Err(e) => {
                    warn!("Couldn't decode savestate: {:?}", e);
                    return Err(())
                }
            };

        let gl_is_valid = self.retrogl.is_valid();

        // Save the disc before we replace everything
        let disc = self.cpu.interconnect_mut().cdrom_mut().remove_disc();

        self.cpu = cpu;
        self.retrogl = retrogl;
        self.video_clock = video_clock;
        self.shared_state = shared_state;

        self.cpu.interconnect_mut().cdrom_mut().set_disc(disc);

        // XXX TODO: reload BIOS and controllers

        // If we had a valid GL context before the load we can
        // directly reload everything. Otherwise it'll be done when
        // the frontend calls context_reset
        if gl_is_valid {
            self.retrogl.context_reset();
        }

        info!("Savestate load successful");

        Ok(())
    }

    fn load_disc(disc: &Path) -> Result<(Cpu, VideoClock), ()> {
        let image =
            match Cue::new(disc) {
                Ok(c) => c,
                Err(e) => {
                    error!("Couldn't load {}: {}", disc.to_string_lossy(), e);
                    return Err(());
                }
            };

        let disc =
            match Disc::new(Box::new(image)) {
                Ok(d) => d,
                Err(e) => {
                    error!("Couldn't load {}: {}", disc.to_string_lossy(), e);
                    return Err(());
                }
            };

        let region = disc.region();

        info!("Detected disc region: {:?}", region);

        let bios =
            match Context::find_bios(region) {
                Some(b) => b,
                None => {
                    error!("Couldn't find a BIOS, bailing out");
                    return Err(());
                }
            };

        let video_clock =
            match region {
                Region::Europe => VideoClock::Pal,
                Region::NorthAmerica => VideoClock::Ntsc,
                Region::Japan => VideoClock::Ntsc,
            };

        // If we're asked to boot straight to the BIOS menu we pretend
        // no disc is present.
        let disc =
            if CoreVariables::bios_menu() {
                None
            } else {
                Some(disc)
            };

        let gpu = Gpu::new(video_clock);
        let inter = Interconnect::new(bios, gpu, disc);

        Ok((Cpu::new(inter), video_clock))
    }

    /// Attempt to find a BIOS for `region` in the system directory
    fn find_bios(region: Region) -> Option<Bios> {
        let system_directory =
            match libretro::get_system_directory() {
                Some(dir) => dir,
                // libretro.h says that when the system directory is not
                // provided "it's up to the implementation to find a
                // suitable directory" but I'm not sure what to put
                // here. Maybe "."? I'd rather give an explicit error
                // message instead.
                None => {
                    error!("The frontend didn't give us a system directory, \
                            no BIOS can be loaded");
                    return None;
                }
            };

        info!("Looking for a BIOS for region {:?} in {:?}",
              region,
              system_directory);

        let dir =
            match ::std::fs::read_dir(&system_directory) {
                Ok(d) => d,
                Err(e) => {
                    error!("Can't read directory {:?}: {}",
                           system_directory, e);
                    return None;
                }
            };

        for entry in dir {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    match entry.metadata() {
                        Ok(md) => {
                            if !md.is_file() {
                                debug!("Ignoring {:?}: not a file", path);
                            } else if md.len() != BIOS_SIZE as u64 {
                                debug!("Ignoring {:?}: bad size", path);
                            } else {
                                let bios = Context::try_bios(region, &path);

                                if bios.is_some() {
                                    // Found a valid BIOS!
                                    return bios;
                                }
                            }
                        }
                        Err(e) =>
                            warn!("Ignoring {:?}: can't get file metadata: {}",
                                  path, e)
                    }
                }
                Err(e) => warn!("Error while reading directory: {}", e),
            }
        }

        None
    }

    /// Attempt to read and load the BIOS at `path`
    fn try_bios(region: Region, path: &Path) -> Option<Bios> {

        let mut file =
            match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    warn!("Can't open {:?}: {}", path, e);
                    return None;
                }
            };

        // Load the BIOS
        let mut data = Box::new([0; BIOS_SIZE]);
        let mut nread = 0;

        while nread < BIOS_SIZE {
            nread +=
                match file.read(&mut data[nread..]) {
                    Ok(0) => {
                        warn!("Short read while loading {:?}", path);
                        return None;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        warn!("Error while reading {:?}: {}", path, e);
                        return None;
                    }
                };
        }

        match Bios::new(data) {
            Some(bios) => {
                let md = bios.metadata();

                if md.known_bad {
                    warn!("Ignoring {:?}: known bad dump", path);
                    None
                } else if md.region != region {
                    info!("Ignoring {:?}: bad region ({:?})", path, md.region);
                    None
                } else {
                    info!("Using BIOS {:?} ({:?}, version {}.{})",
                          path,
                          md.region,
                          md.version_major,
                          md.version_minor);
                    Some(bios)
                }
            }
            None => {
                debug!("Ignoring {:?}: not a known PlayStation BIOS", path);
                None
            }
        }
    }

    fn poll_controllers(&mut self) {
        // XXX we only support pad 0 for now
        let pad = &mut *self.cpu.interconnect_mut()
            .pad_memcard_mut()
            .pad_profiles()[0];

        for &(retrobutton, psxbutton) in &BUTTON_MAP {
            let state =
                if libretro::button_pressed(0, retrobutton) {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };

            pad.set_button_state(psxbutton, state);
        }
    }
}

impl libretro::Context for Context {

    fn render_frame(&mut self) {

        self.frame_count += 1;

        self.poll_controllers();

        let cpu = &mut self.cpu;
        let shared_state = &mut self.shared_state;
        let debugger = &mut self.debugger;

        if libretro::key_pressed(0, libretro::Key::Pause) {
            // Trigger the debugger
            debugger.trigger_break();
        }

        self.retrogl.render_frame(|renderer| {
            cpu.run_until_next_frame(debugger, shared_state, renderer);
        });

        if self.monitor_internal_fps {
            // In order to compute the internal game framerate we
            // monitor whether the display coordinates have
            // changed. Since most games use double buffering that
            // should effectively give us the internal framerate.
            let display_start = cpu.interconnect().gpu().display_vram_start();

            if display_start != self.prev_display_start {
                self.prev_display_start = display_start;
                self.internal_frame_count += 1;
            }

            if self.frame_count % INTERNAL_FPS_SAMPLE_PERIOD == 0 {
                // We compute the internal FPS relative to the
                // full-speed video output FPS.
                let video_fps = video_output_framerate(self.video_clock);

                let internal_fps =
                    (self.internal_frame_count as f32 * video_fps)
                    / INTERNAL_FPS_SAMPLE_PERIOD as f32;

                libretro_message!(100, "Internal FPS: {:.2}", internal_fps);

                self.internal_frame_count = 0;
            }
        }
    }

    fn get_system_av_info(&self) -> libretro::SystemAvInfo {
        let upscaling = CoreVariables::internal_upscale_factor();

        get_av_info(self.video_clock, upscaling)
    }

    fn refresh_variables(&mut self) {
        self.monitor_internal_fps = CoreVariables::display_internal_fps();

        self.retrogl.refresh_variables();
    }

    fn reset(&mut self) {
        match Context::load_disc(&self.disc_path) {
            Ok((cpu, video_clock)) => {
                info!("Game reset");
                self.cpu = cpu;
                self.video_clock = video_clock;
                self.shared_state = SharedState::new();
            },
            Err(_) => warn!("Couldn't reset game"),
        }
    }

    fn gl_context_reset(&mut self) {
        self.retrogl.context_reset();
    }

    fn gl_context_destroy(&mut self) {
        self.retrogl.context_destroy();
    }

    fn serialize_size(&self) -> usize {
        self.savestate_max_len
    }

    fn serialize(&self, mut buf: &mut [u8]) -> Result<(), ()> {
        self.save_state(&mut buf)
    }

    fn unserialize(&mut self, mut buf: &[u8]) -> Result<(), ()> {
        self.load_state(&mut buf)
    }
}

impl Encodable for Context {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("Context", 4, |s| {
            try!(s.emit_struct_field("cpu", 0,
                                     |s| self.cpu.encode(s)));
            try!(s.emit_struct_field("retrogl", 1,
                                     |s| self.retrogl.encode(s)));
            try!(s.emit_struct_field("video_clock", 2,
                                     |s| self.video_clock.encode(s)));
            try!(s.emit_struct_field("shared_state", 3,
                                     |s| self.shared_state.encode(s)));

            Ok(())
        })
    }
}

/// Init function, guaranteed called only once (unlike `retro_init`)
fn init() {
    retrolog::init();
}

/// Called when a game is loaded and a new context must be built
fn load_game(disc: PathBuf) -> Option<Box<libretro::Context>> {
    info!("Loading {:?}", disc);

    Context::new(&disc).ok()
        .map(|c| Box::new(c) as Box<libretro::Context>)
}

libretro_variables!(
    struct CoreVariables (prefix = "rustation") {
        internal_upscale_factor: u32, parse_upscale
            => "Internal upscaling factor; \
                1x (native)|2x|3x|4x|5x|6x|7x|8x|9x|10x",
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

fn init_variables() {
    CoreVariables::register();
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

/// Libretro to PlayStation button mapping. Libretro's mapping is
/// based on the SNES controller so libretro's A button matches the
/// PlayStation's Circle button.
const BUTTON_MAP: [(libretro::JoyPadButton, Button); 14] =
    [(libretro::JoyPadButton::Up, Button::DUp),
     (libretro::JoyPadButton::Down, Button::DDown),
     (libretro::JoyPadButton::Left, Button::DLeft),
     (libretro::JoyPadButton::Right, Button::DRight),
     (libretro::JoyPadButton::Start, Button::Start),
     (libretro::JoyPadButton::Select, Button::Select),
     (libretro::JoyPadButton::A, Button::Circle),
     (libretro::JoyPadButton::B, Button::Cross),
     (libretro::JoyPadButton::Y, Button::Square),
     (libretro::JoyPadButton::X, Button::Triangle),
     (libretro::JoyPadButton::L, Button::L1),
     (libretro::JoyPadButton::R, Button::R1),
     (libretro::JoyPadButton::L2, Button::L2),
     (libretro::JoyPadButton::R2, Button::R2)];

/// Number of output frames over which the internal FPS is averaged
const INTERNAL_FPS_SAMPLE_PERIOD: u32 = 32;
