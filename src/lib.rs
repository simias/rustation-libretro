#[macro_use]
pub mod libretro;
mod retrogl;
mod retrolog;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;

use libc::{c_char, c_uint};

use rustation::cdrom::disc::{Disc, Region};
use rustation::bios::{Bios, BIOS_SIZE};
use rustation::gpu::{Gpu, VideoClock};
use rustation::memory::Interconnect;
use rustation::cpu::Cpu;
use rustation::padmemcard::gamepad::{Button, ButtonState};
use rustation::shared::SharedState;

extern crate libc;
extern crate gl;
#[macro_use]
extern crate log;
extern crate rustation;
extern crate arrayvec;

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
    cpu: Cpu,
    shared_state: SharedState,
    disc_path: PathBuf,
    video_clock: VideoClock,
}

impl Context {
    fn new(disc: &Path) -> Result<Context, ()> {

        let (cpu, video_clock) = try!(Context::load_disc(disc));
        let shared_state = SharedState::new();
        let retrogl = try!(retrogl::RetroGl::new(video_clock));

        Ok(Context {
            retrogl: retrogl,
            cpu: cpu,
            shared_state: shared_state,
            disc_path: disc.to_path_buf(),
            video_clock: video_clock,
        })
    }

    fn load_disc(disc: &Path) -> Result<(Cpu, VideoClock), ()> {
        let disc =
            match Disc::from_path(&disc) {
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
        let pad = &mut *self.cpu.pad_profiles()[0];

        for &(retrobutton, psxbutton) in &BUTTON_MAP {
            let state =
                if libretro::button_pressed(retrobutton) {
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

        self.poll_controllers();

        let cpu = &mut self.cpu;
        let shared_state = &mut self.shared_state;

        self.retrogl.render_frame(|renderer| {
            cpu.run_until_next_frame(shared_state, renderer);
        });
    }

    fn get_system_av_info(&self) -> libretro::SystemAvInfo {
        let upscaling = CoreVariables::internal_upscale_factor();

        get_av_info(self.video_clock, upscaling)
    }

    fn refresh_variables(&mut self) {
        self.retrogl.refresh_variables();
    }

    fn reset(&mut self) {

        match Context::load_disc(&self.disc_path) {
            Ok((cpu, video_clock)) => {
                info!("Game reset");
                self.cpu = cpu;
                self.video_clock = video_clock;
                self.shared_state.reset();
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
        internal_upscale_factor: u32
            => "Internal upscaling factor; 1|2|3|4|5|6|7|8|9|10",
        internal_color_depth: u8
            => "Internal color depth; 16|32",
        scale_dither: bool
            => "Scale dithering pattern with internal resolution; false|true",
        wireframe: bool
            => "Wireframe mode; false|true",
        bios_menu: bool
            => "Boot to BIOS menu; false|true",
    });

fn init_variables() {
    CoreVariables::register();
}

fn get_av_info(std: VideoClock, upscaling: u32) -> libretro::SystemAvInfo {

    // Maximum resolution supported by the PlayStation video
    // output is 640x480
    let max_width = (640 * upscaling) as c_uint;
    let max_height = (480 * upscaling) as c_uint;

    // Precise FPS values for the video output. It's actually possible
    // to configure the PlayStation GPU to output with NTSC timings
    // with the PAL clock (and vice-versa) which would make this code
    // invalid but it wouldn't make a lot of sense for a game to do
    // that.
    let fps =
        match std {
            // 53.690MHz GPU clock frequency, 263 lines per field,
            // 3413 cycles per line
            VideoClock::Ntsc => 59.81,
            // 53.222MHz GPU clock frequency, 314 lines per field,
            // 3406 cycles per line
            VideoClock::Pal => 49.76,
        };

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
            fps: fps,
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
