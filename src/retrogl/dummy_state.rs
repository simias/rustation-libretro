use rustation::gpu::renderer::{Renderer, Vertex};
use retrogl::State;

/// RetroGL state when no OpenGL context is available. It just holds
/// the data necessary to restart the emulation when a new context is
/// provided.
pub struct DummyState {
    xres: u16,
    yres: u16,
}

impl DummyState {
    pub fn new() -> DummyState {
        DummyState {
            xres: 1024,
            yres: 512,
        }
    }

    pub fn from_state(state: &State) -> DummyState {
        DummyState {
            xres: state.xres(),
            yres: state.yres(),
        }
    }
}

impl State for DummyState {
    fn xres(&self) -> u16 {
        self.xres
    }

    fn yres(&self) -> u16 {
        self.yres
    }

    fn renderer_mut(&mut self) -> &mut Renderer {
        &mut *self
    }

    fn prepare_render(&mut self) {
    }

    fn cleanup_render(&mut self) {
    }

    fn display(&mut self) {
    }

}

impl Renderer for DummyState {
    fn push_triangle(&mut self, _: &[Vertex; 3]) {
        warn!("Dummy push_triangle called");
    }
}
