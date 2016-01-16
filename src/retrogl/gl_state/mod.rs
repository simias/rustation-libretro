use gl;
use gl::types::{GLuint, GLint, GLsizei};
use arrayvec::ArrayVec;
use libc::c_uint;
use rustation::gpu::renderer::{Renderer, Vertex, PrimitiveAttributes};
use rustation::gpu::renderer::{TextureDepth, BlendMode};
use rustation::gpu::{VRAM_WIDTH_PIXELS, VRAM_HEIGHT};

use retrogl::{State, DrawConfig};
use retrogl::error::Error;
use retrogl::buffer::DrawBuffer;
use retrogl::shader::{Shader, ShaderType};
use retrogl::program::Program;
use retrogl::types::GlType;
use retrogl::texture::Texture;
use retrogl::framebuffer::Framebuffer;

use libretro;

pub struct GlState {
    command_buffer: DrawBuffer<CommandVertex>,
    output_buffer: DrawBuffer<OutputVertex>,
    /// Texture used to store the VRAM for texture mapping
    config: DrawConfig,
    /// Framebuffer used as a shader input for texturing draw commands
    fb_texture: Texture,
    /// Framebuffer used as an output when running draw commands
    fb_out: Texture,
    /// Current resolution of the frontend's framebuffer
    frontend_resolution: (u32, u32),
    vram_debug: bool,
}

impl GlState {
    pub fn from_config(config: DrawConfig) -> Result<GlState, Error> {
        info!("Building OpenGL state");

        let vs = try!(Shader::new(include_str!("shaders/command_vertex.glsl"),
                                  ShaderType::Vertex));

        let fs = try!(Shader::new(include_str!("shaders/command_fragment.glsl"),
                                  ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        let command_buffer = try!(DrawBuffer::new(2048, program));


        let vs = try!(Shader::new(include_str!("shaders/output_vertex.glsl"),
                                  ShaderType::Vertex));

        let fs = try!(Shader::new(include_str!("shaders/output_fragment.glsl"),
                                  ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        let output_buffer = try!(DrawBuffer::new(4, program));

        let fb_texture = try!(Texture::new(VRAM_WIDTH_PIXELS as u32,
                                           VRAM_HEIGHT as u32,
                                           gl::RGB5_A1));

        match Framebuffer::new(&fb_texture) {
            Ok(_) => unsafe {
                // Clear the FB texture with an arbitrary color. The
                // VRAM's contents on startup are undefined
                gl::ClearColor(1.0, 0.5, 0.2, 0.);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            },
            Err(e) => panic!("Can't create framebuffer: {:?}", e),
        }

        // XXX support increased resolution and color depth
        let fb_out = try!(Texture::new(VRAM_WIDTH_PIXELS as u32,
                                       VRAM_HEIGHT as u32,
                                       gl::RGB5_A1));

        match Framebuffer::new(&fb_out) {
            Ok(_) => unsafe {
                // Clear the FB texture with an arbitrary color. The
                // VRAM's contents on startup are undefined
                gl::ClearColor(1.0, 0.5, 0.2, 0.);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            },
            Err(e) => panic!("Can't create framebuffer: {:?}", e),
        }

        Ok(GlState {
            command_buffer: command_buffer,
            output_buffer: output_buffer,
            config: config,
            fb_texture: fb_texture,
            fb_out: fb_out,
            vram_debug: true,
            frontend_resolution: (0, 0),
        })
    }

    fn draw(&mut self) -> Result<(), Error> {

        unsafe {
            // XXX No semi-transparency support for now
            gl::BlendFuncSeparate(gl::ONE,
                                  gl::ZERO,
                                  gl::ONE,
                                  gl::ZERO)
        }

        let (x, y) = self.config.draw_offset;

        try!(self.command_buffer.program().uniform2i("offset",
                                                     x as GLint,
                                                     y as GLint));

        // We use texture unit 0
        try!(self.command_buffer.program().uniform1i("fb_texture", 0));

        // Bind the out framebuffer
        let _fb = Framebuffer::new(&self.fb_out);

        try!(self.command_buffer.draw(gl::TRIANGLES));

        self.command_buffer.clear()
    }

    fn apply_scissor(&mut self) {
        let (x, y) = self.config.draw_area_top_left;
        let (w, h) = self.config.draw_area_resolution;

        unsafe {
            gl::Scissor(x as GLint, y as GLint, w as GLsizei, h as GLsizei);
        }
    }

    fn bind_libretro_framebuffer(&mut self) {
        let (f_w, f_h) = self.frontend_resolution;
        let (w, h) = self.config.display_resolution;

        // XXX scale w and h when implementing increased internal
        // resolution
        let w = w as u32;
        let h = h as u32;

        if w != f_w || h != f_h {
            let geometry = libretro::GameGeometry {
                base_width: w as c_uint,
                base_height: h as c_uint,
                max_width: 640,
                max_height: 576,
                // Is this accurate?
                aspect_ratio: 4./3.,
            };

            libretro::set_geometry(&geometry);

            // We need to change the frontend's resolution
            self.frontend_resolution = (w, h);
        }

        // Bind the output framebuffer provided by the frontend
        let fbo = libretro::hw_context::get_current_framebuffer() as GLuint;

        unsafe {
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo);
            gl::Viewport(0, 0, w as GLsizei, h as GLsizei);
        }
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

        unsafe {
            // XXX no semi-transparency for now
            gl::Disable(gl::BLEND);
            gl::Enable(gl::SCISSOR_TEST);
        }

        self.apply_scissor();

        // Bind `fb_texture` to texture unit 0
        self.fb_texture.bind(gl::TEXTURE0);
    }

    fn finalize_frame(&mut self) {
        // Draw pending commands
        self.draw().unwrap();

        // We can now render to the frontend's buffer.
        self.bind_libretro_framebuffer();

        // Bind `fb_out` to texture unit 1
        self.fb_out.bind(gl::TEXTURE1);

        // First we draw the visible part of fb_out
        unsafe {
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
        }

        let (fb_x_start, fb_y_start) = self.config.display_top_left;
        let (fb_width, fb_height) = self.config.display_resolution;

        let fb_x_end = fb_x_start + fb_width;
        let fb_y_end = fb_y_start + fb_height;

        self.output_buffer.clear().unwrap();
        self.output_buffer.push_slice(
            &[OutputVertex { position: [-1., -1.],
                             fb_coord: [fb_x_start, fb_y_end] },
              OutputVertex { position: [1., -1.],
                             fb_coord: [fb_x_end, fb_y_end] },
              OutputVertex { position: [-1., 1.],
                             fb_coord: [fb_x_start, fb_y_start] },
              OutputVertex { position: [1., 1.],
                             fb_coord: [fb_x_end, fb_y_start] }])
            .unwrap();

        self.output_buffer.program().uniform1i("fb", 1).unwrap();
        self.output_buffer.draw(gl::TRIANGLE_STRIP).unwrap();

        if self.vram_debug {
            // Draw VRAM insert
            unsafe {
                // Enable alpha blending for the VRAM display
                gl::Enable(gl::BLEND);
                gl::BlendColor(0., 0., 0., 0.7);
                gl::BlendEquationSeparate(gl::FUNC_ADD, gl::FUNC_ADD);
                gl::BlendFuncSeparate(gl::CONSTANT_ALPHA,
                                      gl::ONE_MINUS_CONSTANT_ALPHA,
                                      gl::ONE,
                                      gl::ZERO);
            }

            self.output_buffer.clear().unwrap();
            self.output_buffer.push_slice(
                &[OutputVertex { position: [0., -1.0],
                                 fb_coord: [0, 512] },
                  OutputVertex { position: [1.0, -1.0],
                                 fb_coord: [1024, 512] },
                  OutputVertex { position: [0., -0.5],
                                 fb_coord: [0, 0] },
                  OutputVertex { position: [1.0, -0.5],
                                 fb_coord: [1024, 0] }])
                .unwrap();

            // Bind fb texture
            self.output_buffer.program().uniform1i("fb", 0).unwrap();
            self.output_buffer.draw(gl::TRIANGLE_STRIP).unwrap();
        }

        // Cleanup OpenGL context before returning to the frontend
        unsafe {
            gl::Disable(gl::BLEND);
            gl::BlendColor(0., 0., 0., 0.);
            gl::BlendEquationSeparate(gl::FUNC_ADD, gl::FUNC_ADD);
            gl::BlendFuncSeparate(gl::ONE,
                                  gl::ZERO,
                                  gl::ONE,
                                  gl::ZERO);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
            gl::BindVertexArray(0);
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0);
        }

        libretro::gl_frame_done(self.frontend_resolution.0,
                                self.frontend_resolution.1)
    }
}

impl Renderer for GlState {
    fn set_draw_offset(&mut self, x: i16, y: i16) {
        self.config.draw_offset = (x, y)
    }

    fn set_draw_area(&mut self, top_left: (u16, u16), resolution: (u16, u16)) {
        // Finish drawing anything in the current area
        self.draw().unwrap();

        self.config.draw_area_top_left = top_left;
        self.config.draw_area_resolution = resolution;

        self.apply_scissor();
    }

    fn set_display_mode(&mut self,
                        top_left: (u16, u16),
                        resolution: (u16, u16),
                        depth_24bpp: bool) {
        self.config.display_top_left = top_left;
        self.config.display_resolution = resolution;
        self.config.display_24bpp = depth_24bpp;
    }

    fn push_line(&mut self,
                 _: &PrimitiveAttributes,
                 _: &[Vertex; 2]) {
        unimplemented!()
    }

    fn push_triangle(&mut self,
                     attributes: &PrimitiveAttributes,
                     vertices: &[Vertex; 3]) {
        if self.command_buffer.remaining_capacity() < 3 {
            self.draw().unwrap();
        }

        let v: ArrayVec<[_; 3]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(attributes, v))
            .collect();

        self.command_buffer.push_slice(&v).unwrap();
    }

    fn push_quad(&mut self,
                 attributes: &PrimitiveAttributes,
                 vertices: &[Vertex; 4]) {
        if self.command_buffer.remaining_capacity() < 6 {
            self.draw().unwrap();
        }

        let v: ArrayVec<[_; 4]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(attributes, v))
            .collect();

        self.command_buffer.push_slice(&v[0..3]).unwrap();
        self.command_buffer.push_slice(&v[1..4]).unwrap();
    }

    fn load_image(&mut self,
                  top_left: (u16, u16),
                  resolution: (u16, u16),
                  pixel_buffer: &[u16]) {
        self.fb_texture.set_sub_image(top_left,
                                      resolution,
                                      gl::RGBA,
                                      gl::UNSIGNED_SHORT_1_5_5_5_REV,
                                      pixel_buffer).unwrap();

        // XXX update target as well (in case the game uploads
        // graphics directly to the work buffer)
    }
}

#[derive(Default)]
struct CommandVertex {
    /// Position in PlayStation VRAM coordinates
    position: [i16; 2],
    /// RGB color, 8bits per component
    color: [u8; 3],
    /// Texture coordinates within the page
    texture_coord: [u16; 2],
    /// Texture page (base offset in VRAM used for texture lookup)
    texture_page: [u16; 2],
    /// Color Look-Up Table (palette) coordinates in VRAM
    clut: [u16; 2],
    /// Blending mode: 0: no texture, 1: raw-texture, 2: texture-blended
    texture_blend_mode: u8,
    /// Right shift from 16bits: 0 for 16bpp textures, 1 for 8bpp, 2
    /// for 4bpp
    depth_shift: u8,
    /// True if dithering is enabled for this primitive
    dither: u8,
}

implement_vertex!(CommandVertex,
                  position, color, texture_page,
                  texture_coord, clut, texture_blend_mode,
                  depth_shift, dither);

impl CommandVertex {
    fn from_vertex(attributes: &PrimitiveAttributes,
                   v: &Vertex) -> CommandVertex {
        CommandVertex {
            position: v.position,
            color: v.color,
            texture_coord: v.texture_coord,
            texture_page: attributes.texture_page,
            clut: attributes.clut,
            texture_blend_mode: match attributes.blend_mode {
                BlendMode::None => 0,
                BlendMode::Raw => 1,
                BlendMode::Blended => 2,
            },
            depth_shift: match attributes.texture_depth {
                TextureDepth::T4Bpp => 2,
                TextureDepth::T8Bpp => 1,
                TextureDepth::T16Bpp => 0,
            },
            dither: attributes.dither as u8,
        }
    }
}

struct OutputVertex {
    /// Vertex position on the screen
    position: [f32; 2],
    /// Corresponding coordinate in the framebuffer
    fb_coord: [u16; 2],
}

implement_vertex!(OutputVertex,
                  position, fb_coord);
