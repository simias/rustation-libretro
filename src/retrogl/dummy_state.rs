use rustation::gpu::renderer::{Renderer, Vertex, PrimitiveAttributes};
use retrogl::{State, DrawConfig};

/// RetroGL state when no OpenGL context is available. It just holds
/// the data necessary to restart the emulation when a new context is
/// provided.
pub struct DummyState {
    config: DrawConfig,
}

impl DummyState {
    pub fn from_config(config: DrawConfig) -> DummyState {
        DummyState {
            config: config,
        }
    }
}

impl State for DummyState {
    fn draw_config(&self) -> &DrawConfig {
        &self.config
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
    fn set_draw_offset(&mut self, x: i16, y: i16) {
        self.config.draw_offset = (x, y)
    }

    fn push_line(&mut self, _: &PrimitiveAttributes, _: &[Vertex; 2]) {
        warn!("Dummy push_line called");
    }

    fn push_triangle(&mut self, _: &PrimitiveAttributes, _: &[Vertex; 3]) {
        warn!("Dummy push_triangle called");
    }

    fn push_quad(&mut self, _: &PrimitiveAttributes, _: &[Vertex; 4]) {
        warn!("Dummy push_quad called");
    }

    fn load_image(&mut self, _: (u16, u16), _: (u16, u16), _: &[u16]) {
        warn!("Dummy load_image called");
    }
}
