// XXX temporarily necessary to remove annoying warnings about the
// cstring! macro in rustc 1.10.0 which don't have a simple
// workaround. Will be removed once we find a better way to silence
// them.
#![allow(const_err)]

#[macro_use]
pub mod libretro;
#[macro_use]
mod retrogl;
mod retrolog;
mod renderer;
mod savestate;
mod debugger;
mod vcd;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

use libc::{c_char, c_uint};

use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use rustation::cdrom::disc::{Disc, Region};
use rustation::bios::{Bios, BIOS_SIZE};
use rustation::bios::db::Metadata;
use rustation::gpu::{Gpu, VideoClock};
use rustation::memory::Interconnect;
use rustation::cpu::Cpu;
use rustation::padmemcard::gamepad::{Button, ButtonState, DigitalProfile};
use rustation::shared::SharedState;
use rustation::parallel_io::exe_loader;
use rustation::tracer;

use cdimage::cue::Cue;

use debugger::Debugger;

#[macro_use]
extern crate log;
extern crate libc;
extern crate gl;
extern crate rustation;
extern crate arrayvec;
extern crate cdimage;
extern crate rustc_serialize;
extern crate time;

/// Static system information sent to the frontend on request
const SYSTEM_INFO: libretro::SystemInfo = libretro::SystemInfo {
    library_name: cstring!("Rustation"),
    library_version: rustation::VERSION_CSTR as *const _ as *const c_char,
    valid_extensions: cstring!("cue|exe|psexe|psx"),
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
    /// When true the internal FPS monitoring in enabled
    monitor_internal_fps: bool,
    /// Cached value for the maximum savestate size in bytes
    savestate_max_len: usize,
    /// If true we log the counters at the end of each frame
    log_frame_counters: bool,
    /// If true we trigger the debugger when Pause/Break is pressed
    debug_on_key: bool,
}

impl Context {
    fn new(disc: &Path) -> Result<Context, ()> {

        let (mut cpu, video_clock) =
            match exe_loader::ExeLoader::load_file(disc) {
                Ok(l) => try!(Context::load_exe(l)),
                // Not an EXE, load as a disc
                Err(exe_loader::Error::UnknownFormat) =>
                    try!(Context::load_disc(disc)),
                Err(e) => {
                    error!("Couldn't load EXE file: {:?}", e);
                    return Err(())
                }
            };

        let shared_state = SharedState::new();
        let retrogl = try!(retrogl::RetroGl::new(video_clock));

        if CoreVariables::enable_debug_uart() {
            let result =
                cpu.interconnect_mut().bios_mut().enable_debug_uart();

            match result {
                Ok(_) => info!("BIOS patched to enable debug UART"),
                Err(_) => warn!("Couldn't patch BIOS to enable debug UART"),
            }
        }

        let mut context =
            Context {
                retrogl: retrogl,
                cpu: cpu,
                shared_state: shared_state,
                debugger: Debugger::new(),
                disc_path: disc.to_path_buf(),
                video_clock: video_clock,
                monitor_internal_fps: false,
                savestate_max_len: 0,
                log_frame_counters: false,
                debug_on_key: false,
            };

        libretro::Context::refresh_variables(&mut context);

        let max_len = try!(context.compute_savestate_max_length());

        context.savestate_max_len = max_len;

        context.setup_controllers();

        if CoreVariables::debug_on_reset() {
            context.trigger_break();
        }

        Ok(context)
    }

    /// Initialize the controllers connected to the emulated console
    fn setup_controllers(&mut self) {
        // XXX for now I only hardcode a digital pad in slot 1
        // (leaving slot 0 disconnected).
        self.cpu.interconnect_mut()
            .pad_memcard_mut()
            .gamepads_mut()[0]
            .set_profile(Box::new(DigitalProfile::new()));
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

        // The savestate doesn't contain the BIOS, only the metadata
        // describing which BIOS was used when the savestate was made
        // (in order to save space and not redistribute the BIOS with
        // savestate files). So let's find it back and reload it.
        let bios_md = self.cpu.interconnect().bios().metadata();

        // Convert sha256 to a hex string for pretty printing
        let sha256_hex: String =
            bios_md.sha256.iter()
            .fold(String::new(), |s, b| s + &format!("{:02x}", b));

        info!("Loading savestate BIOS: {:?} (SHA256: {})",
              bios_md, sha256_hex);

        let bios =
            match Context::find_bios(|md| { md.sha256 == bios_md.sha256 }) {
                Some(b) => b,
                None => {
                    error!("Couldn't find the savestate BIOS, bailing out");
                    return Err(());
                }
            };

        let gl_is_valid = self.retrogl.is_valid();

        // Save the disc before we replace everything
        let disc = self.cpu.interconnect_mut().cdrom_mut().remove_disc();

        self.cpu = cpu;
        self.retrogl = retrogl;
        self.video_clock = video_clock;
        self.shared_state = shared_state;

        self.cpu.interconnect_mut().set_bios(bios);
        self.cpu.interconnect_mut().cdrom_mut().set_disc(disc);

        self.setup_controllers();

        // If we had a valid GL context before the load we can
        // directly reload everything. Otherwise it'll be done when
        // the frontend calls context_reset
        if gl_is_valid {
            self.retrogl.context_reset();
        }

        info!("Savestate load successful");

        Ok(())
    }

    fn load_exe(loader: exe_loader::ExeLoader)
                -> Result<(Cpu, VideoClock), ()> {
        let region =
            match loader.region() {
                Some(r) => {
                    info!("Detected EXE region: {:?}", r);
                    r
                }
                None => {
                    warn!("Couldn't establish EXE file region, \
                           defaulting to NorthAmerica");
                    Region::NorthAmerica
                }
            };

        // In order for the EXE loader to word correctly without any
        // disc we need to patch the BIOS, so let's make sure that the
        // animation_jump_hook is available
        let bios_predicate = |md: &Metadata| {
            md.region == region && md.animation_jump_hook.is_some()
        };

        let mut bios =
            match Context::find_bios(bios_predicate) {
                Some(b) => b,
                None => {
                    error!("Couldn't find a BIOS, bailing out");
                    return Err(());
                }
            };

        if let Err(_) = loader.patch_bios(&mut bios) {
             error!("EXE loader couldn't patch the BIOS, giving up");
             return Err(());
        }

        let video_clock =
            match region {
                Region::Europe => VideoClock::Pal,
                Region::NorthAmerica => VideoClock::Ntsc,
                Region::Japan => VideoClock::Ntsc,
            };

        let gpu = Gpu::new(video_clock);
        let mut inter = Interconnect::new(bios, gpu, None);

        // Plug the EXE loader in the Parallel I/O port
        inter.parallel_io_mut().set_module(Box::new(loader));

        Ok((Cpu::new(inter), video_clock))
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

        let serial = disc.serial_number();
        let region = disc.region();

        info!("Disc serial number: {}", serial);
        info!("Detected disc region: {:?}", region);

        let mut bios =
            match Context::find_bios(|md| { md.region == region }) {
                Some(b) => b,
                None => {
                    error!("Couldn't find a BIOS, bailing out");
                    return Err(());
                }
            };

        let bios_menu = CoreVariables::bios_menu();

        // Skipping BIOS animations seems to break the BIOS menu, so
        // we ignore this setting when the menu is requested.
        if CoreVariables::skip_bios_animation() && !bios_menu {
            match bios.patch_boot_animation() {
                Ok(_) => info!("Patched BIOS to skip boot animation"),
                Err(_) => warn!("Failed to patch BIOS to skip boot animations"),
            }
        }

        let video_clock =
            match region {
                Region::Europe => VideoClock::Pal,
                Region::NorthAmerica => VideoClock::Ntsc,
                Region::Japan => VideoClock::Ntsc,
            };

        // If we're asked to boot straight to the BIOS menu we pretend
        // no disc is present.
        let disc =
            if bios_menu {
                None
            } else {
                Some(disc)
            };

        let gpu = Gpu::new(video_clock);
        let inter = Interconnect::new(bios, gpu, disc);

        Ok((Cpu::new(inter), video_clock))
    }

    /// Attempt to find a BIOS for `region` in the system directory
    fn find_bios<F>(predicate: F) -> Option<Bios>
        where F: Fn(&Metadata) -> bool {
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

        info!("Looking for a suitable BIOS in {:?}", system_directory);

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
                                let bios = Context::try_bios(&predicate, &path);

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
    fn try_bios<F>(predicate: F, path: &Path) -> Option<Bios>
        where F: Fn(&Metadata) -> bool {

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

                info!("Found BIOS DB entry for {:?}: {:?}", path, md);

                if md.known_bad {
                    warn!("Ignoring {:?}: known bad dump", path);
                    None
                } else if !predicate(md) {
                    info!("Ignoring {:?}: rejected by predicate", path);
                    None
                } else {
                    info!("Using BIOS {:?} ({:?})", path, md);
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
        let pad = self.cpu.interconnect_mut()
            .pad_memcard_mut()
            .gamepads_mut()[0]
            .profile_mut();

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

    /// Trigger a breakpoint in the debugger
    fn trigger_break(&mut self) {
        rustation::debugger::Debugger::trigger_break(&mut self.debugger);
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if cfg!(feature = "trace") {
            // Dump the trace before destroying everything
            let path = VCD_TRACE_PATH;

            let trace = tracer::remove_trace();

            if trace.is_empty() {
                warn!("Empty trace, ignoring");
            } else {
                info!("Dumping VCD trace file to {}", path);

                let mut vcd_file = File::create(path).unwrap();

                let content = &*self.disc_path.to_string_lossy();

                let bios_md = self.cpu.interconnect().bios().metadata();
                let bios_desc = format!("{:?}", bios_md);

                vcd::dump_trace(&mut vcd_file, content, &bios_desc, trace);
            }
        }
    }
}

impl libretro::Context for Context {

    fn render_frame(&mut self) {
        self.poll_controllers();

        let debug_request =
            self.debug_on_key &&
            libretro::key_pressed(0, libretro::Key::Pause);

        if debug_request {
            self.trigger_break();
        }

        let cpu = &mut self.cpu;
        let shared_state = &mut self.shared_state;
        let debugger = &mut self.debugger;

        self.retrogl.render_frame(|renderer| {
            cpu.run_until_next_frame(debugger, shared_state, renderer);
        });

        let counters = shared_state.counters_mut();

        if self.log_frame_counters {
            debug!("Frame counters:");
            debug!("    CPU interrupt count: {}", counters.cpu_interrupt.get());
        }

        if self.monitor_internal_fps {
            let frame_count = counters.frame.get();

            if frame_count >= INTERNAL_FPS_SAMPLE_PERIOD {
                // We compute the internal FPS relative to the
                // full-speed video output FPS.
                let video_fps = video_output_framerate(self.video_clock);

                let internal_frame_count = counters.framebuffer_swap.get();

                let internal_fps =
                    (internal_frame_count as f32 * video_fps)
                    / INTERNAL_FPS_SAMPLE_PERIOD as f32;

                libretro_message!(100, "Internal FPS: {:.2}", internal_fps);

                counters.frame.reset();
                counters.framebuffer_swap.reset();
            }
        } else {
            // Keep those counters to 0 so that we don't get wild
            // values if logging is enabled.
            counters.frame.reset();
            counters.framebuffer_swap.reset();
        }
    }

    fn get_system_av_info(&self) -> libretro::SystemAvInfo {
        let upscaling = CoreVariables::internal_upscale_factor();

        get_av_info(self.video_clock, upscaling)
    }

    fn refresh_variables(&mut self) {
        self.monitor_internal_fps = CoreVariables::display_internal_fps();
        self.log_frame_counters = CoreVariables::log_frame_counters();
        self.debug_on_key = CoreVariables::debug_on_key();
        self.cpu.set_debug_on_break(CoreVariables::debug_on_break());

        self.retrogl.refresh_variables();
    }

    fn reset(&mut self) {
        match Context::load_disc(&self.disc_path) {
            Ok((cpu, video_clock)) => {
                info!("Game reset");
                self.cpu = cpu;
                self.video_clock = video_clock;
                self.shared_state = SharedState::new();

                if CoreVariables::debug_on_reset() {
                    self.trigger_break();
                }
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
        skip_bios_animation: bool, parse_bool
            => "Skip BIOS boot animations; disabled|enabled",
        display_internal_fps: bool, parse_bool
            => "Display internal FPS; disabled|enabled",
        log_frame_counters: bool, parse_bool
            => "Log frame counters; disabled|enabled",
        enable_debug_uart: bool, parse_bool
            => "Enable debug UART in the BIOS; disabled|enabled",
        debug_on_break: bool, parse_bool
            => "Trigger debugger on BREAK instructions; disabled|enabled",
        debug_on_key: bool, parse_bool
            => "Trigger debugger when Pause/Break is pressed; disabled|enabled",
        debug_on_reset: bool, parse_bool
            => "Trigger debugger when starting or resetting the emulator; \
                disabled|enabled",
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

/// Hardcoded path for the generated VCD file when tracing is
/// enabled. XXX Should probably be changed for Windows, maybe made
/// configurable somehow?
const VCD_TRACE_PATH: &'static str = "/tmp/rustation-trace.vcd";
