use gl;
use gl::types::GLuint;
use rustation::gpu::renderer::Renderer;

use retrogl::State;
use retrogl::error::Error;
use retrogl::buffer::DrawBuffer;
use retrogl::shader::{Shader, ShaderType};
use retrogl::program::Program;

use libretro;

pub struct GlState {
    buffer: DrawBuffer<(f32, f32)>,
    frame: u32,
    xres: u16,
    yres: u16,
}

impl GlState {
    pub fn from_state(state: &State) -> Result<GlState, Error> {
        info!("Building OpenGL state");

        let vs = try!(Shader::new(include_str!("shaders/vertex.glsl"),
                                  ShaderType::Vertex));

        let fs = try!(Shader::new(include_str!("shaders/fragment.glsl"),
                                  ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        let buffer = try!(DrawBuffer::new(128, program));

        Ok(GlState {
            buffer: buffer,
            frame: 0,
            xres: state.xres(),
            yres: state.yres(),
        })
    }

    fn render_frame(&mut self) -> Result<(), Error> {

        try!(self.buffer.push_slice(&[(0., 1.),
                                      (-1., -1.),
                                      (1., -1.),
                                      ]));

        let c = ((self.frame % 0xff) as f32) / 255.;

        try!(self.buffer.program().uniform3f("color", c, 0.5, 0.8));

        try!(self.buffer.draw_triangles());

        self.buffer.clear()
    }
}

impl State for GlState {
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
        // Bind the output framebuffer provided by the frontend
        let fbo = libretro::hw_context::get_current_framebuffer() as GLuint;

        unsafe {
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo);
            gl::Viewport(0, 0, 1024, 512);
        }

        unsafe {
            gl::ClearColor(0.3, 0.4, 0.8, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
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
    fn push_triangle(&mut self) {
        warn!("GL push_triangle called");
    }

    fn display(&mut self) {
        if let Err(e) = self.render_frame() {
            error!("Render frame failed: {:?}", e);
        }
    }
}
