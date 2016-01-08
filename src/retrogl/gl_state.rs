use gl;
use gl::types::{GLuint, GLint};
use rustation::gpu::renderer::{Renderer, Vertex};

use retrogl::{State, DrawConfig};
use retrogl::error::Error;
use retrogl::buffer::DrawBuffer;
use retrogl::shader::{Shader, ShaderType};
use retrogl::program::Program;

use libretro;

pub struct GlState {
    buffer: DrawBuffer<Vertex>,
    config: DrawConfig,
}

impl GlState {
    pub fn from_config(config: DrawConfig) -> Result<GlState, Error> {
        info!("Building OpenGL state");

        let vs = try!(Shader::new(include_str!("shaders/vertex.glsl"),
                                  ShaderType::Vertex));

        let fs = try!(Shader::new(include_str!("shaders/fragment.glsl"),
                                  ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        let buffer = try!(DrawBuffer::new(1024, program));

        Ok(GlState {
            buffer: buffer,
            config: config,
        })
    }

    fn draw(&mut self) -> Result<(), Error> {

        let (x, y) = self.config.draw_offset;

        try!(self.buffer.program().uniform2i("offset",
                                             x as GLint,
                                             y as GLint));

        try!(self.buffer.draw_triangles());

        self.buffer.clear()
    }
}

impl State for GlState {
    fn draw_config(&self) -> &DrawConfig {
        &self.config
    }

    fn renderer_mut(&mut self) -> &mut Renderer {
        &mut *self
    }

    fn prepare_render(&mut self) {
        // Bind the output framebuffer provided by the frontend
        let fbo = libretro::hw_context::get_current_framebuffer() as GLuint;

        unsafe {
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo);
            gl::Viewport(0,
                         0,
                         self.config.xres as i32,
                         self.config.yres as i32);
        }

        unsafe {
            gl::ClearColor(0., 0., 0., 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    fn display(&mut self) {
        if let Err(e) = self.draw() {
            panic!("Render frame failed: {:?}", e);
        }
    }

    fn cleanup_render(&mut self) {
        // Cleanup OpenGL context before returning to the frontend
        unsafe {
            gl::UseProgram(0);
            gl::BindVertexArray(0);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
        }
    }
}

impl Renderer for GlState {
    fn set_draw_offset(&mut self, x: i16, y: i16) {
        self.config.draw_offset = (x, y)
    }

    fn push_triangle(&mut self, vertices: &[Vertex; 3]) {
        if self.buffer.remaining_capacity() < 3 {
            self.draw().unwrap();
        }
        

        self.buffer.push_slice(vertices).unwrap();
    }
}
