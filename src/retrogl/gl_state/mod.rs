use gl;
use gl::types::{GLuint, GLint};
use arrayvec::ArrayVec;
use rustation::gpu::renderer::{Renderer, Vertex, PrimitiveAttributes};

use retrogl::{State, DrawConfig};
use retrogl::error::Error;
use retrogl::buffer::DrawBuffer;
use retrogl::shader::{Shader, ShaderType};
use retrogl::program::Program;
use retrogl::types::GlType;

use libretro;

pub struct GlState {
    buffer: DrawBuffer<CommandVertex>,
    config: DrawConfig,
}

impl GlState {
    pub fn from_config(config: DrawConfig) -> Result<GlState, Error> {
        info!("Building OpenGL state");

        let vs = try!(Shader::new(include_str!("shaders/command_vertex.glsl"),
                                  ShaderType::Vertex));

        let fs = try!(Shader::new(include_str!("shaders/command_fragment.glsl"),
                                  ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        let buffer = try!(DrawBuffer::new(2048, program));

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

    fn push_line(&mut self,
                 _: &PrimitiveAttributes,
                 _: &[Vertex; 2]) {
        unimplemented!()
    }

    fn push_triangle(&mut self,
                     _: &PrimitiveAttributes,
                     vertices: &[Vertex; 3]) {
        if self.buffer.remaining_capacity() < 3 {
            self.draw().unwrap();
        }

        let v: ArrayVec<[_; 3]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(v))
            .collect();

        self.buffer.push_slice(&v).unwrap();
    }

    fn push_quad(&mut self,
                 _: &PrimitiveAttributes,
                 vertices: &[Vertex; 4]) {
        if self.buffer.remaining_capacity() < 6 {
            self.draw().unwrap();
        }

        let v: ArrayVec<[_; 4]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(v))
            .collect();

        self.buffer.push_slice(&v[0..3]).unwrap();
        self.buffer.push_slice(&v[1..4]).unwrap();
    }
}

#[derive(Default)]
struct CommandVertex {
    position: [i16; 2],
    color: [u8; 3],
}

impl CommandVertex {
    fn from_vertex(v: &Vertex) -> CommandVertex {
        CommandVertex {
            position: v.position,
            color: v.color,
        }
    }
}

implement_vertex!(CommandVertex, position, color);
