//! PlayStation OpenGL 3.3 renderer playing nice with libretro

use libretro;
use gl;

use self::buffer::VertexBuffer;
use self::error::Error;
use self::vertex::VertexArrayObject;
use self::program::{Shader, ShaderType};

mod error;
mod buffer;
mod vertex;
mod program;

pub struct RetroGl {
    /// Current horizontal resolution of the video output
    xres: u16,
    /// Current vertical resolution of the video output
    yres: u16,
    /// OpenGL state. None if the context is destroyed/not yet
    /// created.
    state: Option<State>,
}

impl RetroGl {
    pub fn new() -> Option<RetroGl> {
        if !libretro::hw_context::init() {
            error!("Failed to init hardware context");
            return None;
        }

        info!("Initialized RetroGL renderer");

        Some(RetroGl {
            xres: 640,
            yres: 480,
            // Wait until `context_reset` is called
            state: None,
        })
    }

    pub fn context_reset(&mut self) {
        info!("RetroGL context reset");

        // Should I call this at every reset? Does it matter?
        gl::load_with(|s| {
            libretro::hw_context::get_proc_address(s) as *const _
        });

        match State::new() {
            Ok(s) => self.state = Some(s),
            Err(e) => error!("Couldn't create RetroGL state: {:?}", e),
        }
    }

    pub fn context_destroy(&mut self) {
        self.state = None
    }

    pub fn xres(&self) -> u16 {
        self.xres
    }

    pub fn yres(&self) -> u16 {
        self.yres
    }
}

pub struct State {
    vao: VertexArrayObject,
    buffer: VertexBuffer<u32>,
    vertex_shader: Shader,
}

impl State {
    fn new() -> Result<State, Error> {

        let vs = try!(Shader::new(include_str!("shaders/vertex.glsl"),
                                  ShaderType::Vertex));

        let vao = try!(VertexArrayObject::new());

        let buffer = try!(VertexBuffer::new(1024, &vao));

        Ok(State {
            vao: vao,
            buffer: buffer,
            vertex_shader: vs,
        })
    }
}
