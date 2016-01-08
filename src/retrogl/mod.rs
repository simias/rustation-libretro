//! PlayStation OpenGL 3.3 renderer playing nice with libretro

use libretro;
use gl;
use rustation::gpu::renderer::Renderer;

use self::dummy_state::DummyState;
use self::gl_state::GlState;

mod error;
mod buffer;
mod vertex;
mod shader;
mod program;
mod dummy_state;
mod gl_state;

pub struct RetroGl {
    state: Box<State>,
}

impl RetroGl {
    pub fn new() -> Result<RetroGl, ()> {
        if !libretro::hw_context::init() {
            error!("Failed to init hardware context");
            return Err(());
        }

        Ok(RetroGl {
            // No context until `context_reset` is called
            state: Box::new(DummyState::new()),
        })
    }

    pub fn context_reset(&mut self) {
        info!("OpenGL context reset");

        // Should I call this at every reset? Does it matter?
        gl::load_with(|s| {
            libretro::hw_context::get_proc_address(s) as *const _
        });

        match GlState::from_state(self.state()) {
            Ok(s) => self.state = Box::new(s),
            Err(e) => panic!("Couldn't create RetroGL state: {:?}", e),
        }
    }

    pub fn context_destroy(&mut self) {
        self.state = Box::new(DummyState::from_state(self.state()));
    }

    pub fn xres(&self) -> u16 {
        self.state.xres()
    }

    pub fn yres(&self) -> u16 {
        self.state.yres()
    }

    pub fn state(&self) -> &State {
        &*self.state
    }

    pub fn render_frame<F>(&mut self, emulate: F)
        where F: FnOnce(&mut Renderer) {

        self.state.prepare_render();

        emulate(self.state.renderer_mut());

        self.state.display();

        self.state.cleanup_render();
    }
}

pub trait State: Renderer {
    fn xres(&self) -> u16;
    fn yres(&self) -> u16;

    fn prepare_render(&mut self);
    fn display(&mut self);
    fn cleanup_render(&mut self);

    fn renderer_mut(&mut self) -> &mut Renderer;
}
