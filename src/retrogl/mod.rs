//! PlayStation OpenGL 3.3 renderer playing nice with libretro

use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};

use gl;

use rustation::gpu::VideoClock;
use rustation::gpu::renderer::Renderer;
use rustation::gpu::{VRAM_WIDTH_PIXELS, VRAM_HEIGHT};
use CoreVariables;

use libretro;

use renderer::GlRenderer;

#[macro_use]
pub mod vertex;
pub mod error;
pub mod types;
pub mod buffer;
pub mod texture;
pub mod framebuffer;
pub mod shader;
pub mod program;

pub struct RetroGl {
    state: GlState,
    video_clock: VideoClock,
}

impl RetroGl {
    pub fn new(video_clock: VideoClock) -> Result<RetroGl, ()> {
        if !libretro::set_pixel_format(libretro::PixelFormat::Xrgb8888) {
            error!("Can't set pixel format");
            return Err(());
        }

        if !libretro::hw_context::init() {
            error!("Failed to init hardware context");
            return Err(());
        }

        // The VRAM's bootup contents are undefined
        let vram = vec![0xdead; VRAM_PIXELS];

        let config = DrawConfig {
            display_top_left: (0, 0),
            display_resolution: (1024, 512),
            display_24bpp: false,
            draw_area_top_left: (0, 0),
            draw_area_dimensions: (0, 0),
            draw_offset: (0, 0),
            vram: vram,
        };

        Ok(RetroGl {
            // No context until `context_reset` is called
            state: GlState::Invalid(config),
            video_clock: video_clock,
        })
    }

    pub fn context_reset(&mut self) {
        info!("OpenGL context reset");

        // Should I call this at every reset? Does it matter?
        gl::load_with(|s| {
            libretro::hw_context::get_proc_address(s) as *const _
        });

        let config =
            match self.state {
                GlState::Valid(ref r) => r.draw_config().clone(),
                GlState::Invalid(ref c) => c.clone(),
            };

        match GlRenderer::from_config(config) {
            Ok(r) => self.state = GlState::Valid(r),
            Err(e) => panic!("Couldn't create RetroGL state: {:?}", e),
        }
    }

    pub fn context_destroy(&mut self) {
        info!("OpenGL context destroy");

        let config =
        match self.state {
            GlState::Valid(ref r) => r.draw_config().clone(),
            // Looks like we didn't have an OpenGL context anyway...
            GlState::Invalid(_) => return,
        };

        self.state = GlState::Invalid(config);
    }

    pub fn render_frame<F>(&mut self, emulate: F)
        where F: FnOnce(&mut Renderer) {

        let renderer =
            match self.state {
                GlState::Valid(ref mut r) => r,
                GlState::Invalid(_) =>
                    panic!("Attempted to render a frame without GL context"),
            };

        renderer.prepare_render();

        emulate(renderer);

        renderer.finalize_frame();
    }

    pub fn refresh_variables(&mut self) {
        let renderer =
            match self.state {
                GlState::Valid(ref mut r) => r,
                // Nothing to be done if we don't have a GL context
                GlState::Invalid(_) => return,
            };

        let reconfigure_frontend = renderer.refresh_variables();

        if reconfigure_frontend {
            // The resolution has changed, we must tell the frontend
            // to change its format

            let upscaling = CoreVariables::internal_upscale_factor();

            let av_info = ::get_av_info(self.video_clock,
                                        upscaling);

            // This call can potentially (but not necessarily) call
            // `context_destroy` and `context_reset` to reinitialize
            // the entire OpenGL context, so beware.
            let ok = unsafe {
                libretro::set_system_av_info(&av_info)
            };

            if !ok {
                // Some frontends might not support changing the video
                // settings at runtime, if that's the case we continue
                // with the old settings. The new config will be
                // applied on reset.
                warn!("Couldn't change frontend resolution");
                warn!("Try resetting to enable the new configuration");
            }
        }
    }

    /// Return true if we're holding a valid GL context
    pub fn is_valid(&self) -> bool {
        match self.state {
            GlState::Valid(_) => true,
            _ => false,
        }
    }
}

impl Encodable for RetroGl {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_struct("RetroGl", 2, |s| {
            let draw_config =
                match self.state {
                    GlState::Valid(ref r) => r.draw_config(),
                    GlState::Invalid(ref d) => d,
                };

            try!(s.emit_struct_field("draw_config", 0,
                                     |s| draw_config.encode(s)));
            try!(s.emit_struct_field("video_clock", 1,
                                     |s| self.video_clock.encode(s)));

            Ok(())
        })
    }
}

impl Decodable for RetroGl {
    fn decode<D: Decoder>(d: &mut D) -> Result<RetroGl, D::Error> {
        d.read_struct("RetroGl", 2, |d| {
            let draw_config = try!(d.read_struct_field("draw_config", 0,
                                                       Decodable::decode));
            let video_clock = try!(d.read_struct_field("video_clock", 1,
                                                       Decodable::decode));

            Ok(RetroGl{
                state: GlState::Invalid(draw_config),
                video_clock: video_clock,
            })
        })
    }
}

/// State machine dealing with OpenGL context
/// destruction/reconstruction
enum GlState {
    /// OpenGL context is ready
    Valid(GlRenderer),
    /// OpenGL context has been destroy (or is not yet created)
    Invalid(DrawConfig),
}

#[derive(RustcEncodable, RustcDecodable, Clone)]
pub struct DrawConfig {
    pub display_top_left: (u16, u16),
    pub display_resolution: (u16, u16),
    pub display_24bpp: bool,
    pub draw_offset: (i16, i16),
    pub draw_area_top_left: (u16, u16),
    pub draw_area_dimensions: (u16, u16),
    /// VRAM is stored in a Vec instead of a `Box<[u16; VRAM_PIXELS]>`
    /// because with the Box rustc seems to miss an optimization and
    /// puts a temporary array on the stack which overflows on
    /// plaftforms with a shallow stack (Windows for instance).
    pub vram: Vec<u16>,
}

const VRAM_PIXELS: usize = VRAM_WIDTH_PIXELS as usize * VRAM_HEIGHT as usize;
