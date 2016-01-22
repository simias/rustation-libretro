//! PlayStation OpenGL 3.3 renderer playing nice with libretro

use std::rc::Rc;

use libretro;
use gl;
use rustation::gpu::VideoClock;
use rustation::gpu::renderer::Renderer;
use rustation::gpu::{VRAM_WIDTH_PIXELS, VRAM_HEIGHT};
use CoreVariables;

use self::dummy_state::DummyState;
use self::gl_state::GlState;

#[macro_use]
mod vertex;
mod error;
mod types;
mod buffer;
mod texture;
mod framebuffer;
mod shader;
mod program;
mod dummy_state;
mod gl_state;

pub struct RetroGl {
    state: Box<State>,
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

        let config = DrawConfig {
            display_top_left: (0, 0),
            display_resolution: (1024, 512),
            display_24bpp: false,
            draw_area_top_left: (0, 0),
            draw_area_resolution: (0, 0),
            draw_offset: (0, 0),
            vram: Rc::new([0x1f; VRAM_PIXELS]),
        };

        Ok(RetroGl {
            // No context until `context_reset` is called
            state: Box::new(DummyState::from_config(config)),
            video_clock: video_clock,
        })
    }

    pub fn context_reset(&mut self) {
        info!("OpenGL context reset");

        // Should I call this at every reset? Does it matter?
        gl::load_with(|s| {
            libretro::hw_context::get_proc_address(s) as *const _
        });

        let config = self.state.draw_config().clone();

        match GlState::from_config(config) {
            Ok(s) => self.state = Box::new(s),
            Err(e) => panic!("Couldn't create RetroGL state: {:?}", e),
        }
    }

    pub fn context_destroy(&mut self) {
        info!("OpenGL context destroy");

        let config = self.state.draw_config().clone();

        self.state = Box::new(DummyState::from_config(config));
    }

    pub fn render_frame<F>(&mut self, emulate: F)
        where F: FnOnce(&mut Renderer) {

        self.state.prepare_render();

        emulate(self.state.renderer_mut());

        self.state.finalize_frame();
    }

    pub fn refresh_variables(&mut self) {
        let reconfigure_frontend = self.state.refresh_variables();

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
}

pub trait State: Renderer {
    fn draw_config(&self) -> &DrawConfig;
    /// Return `true` if the frontend should be reconfigured
    fn refresh_variables(&mut self) -> bool;

    fn prepare_render(&mut self);
    fn finalize_frame(&mut self);

    fn renderer_mut(&mut self) -> &mut Renderer;
}

#[derive(Clone)]
pub struct DrawConfig {
    display_top_left: (u16, u16),
    display_resolution: (u16, u16),
    display_24bpp: bool,
    draw_offset: (i16, i16),
    draw_area_top_left: (u16, u16),
    draw_area_resolution: (u16, u16),
    vram: Rc<[u16; VRAM_PIXELS]>,
}

const VRAM_PIXELS: usize = VRAM_WIDTH_PIXELS as usize * VRAM_HEIGHT as usize;
