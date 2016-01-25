use std::rc::Rc;
use std::default::Default;

use gl;
use gl::types::{GLuint, GLint, GLsizei, GLenum, GLfloat};
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

use CoreVariables;

use libretro;

pub struct GlState {
    /// Buffer used to handle PlayStation GPU draw commands
    command_buffer: DrawBuffer<CommandVertex>,
    /// Primitive type for the vertices in `command_buffer` (TRIANGLES
    /// or LINES)
    command_draw_mode: GLenum,
    /// Polygon mode (for wireframe)
    command_polygon_mode: GLenum,
    /// Buffer used to draw to the frontend's framebuffer
    output_buffer: DrawBuffer<OutputVertex>,
    /// Buffer used to copy textures from `fb_texture` to `fb_out`
    image_load_buffer: DrawBuffer<ImageLoadVertex>,
    /// Texture used to store the VRAM for texture mapping
    config: DrawConfig,
    /// Framebuffer used as a shader input for texturing draw commands
    fb_texture: Texture,
    /// Framebuffer used as an output when running draw commands
    fb_out: Texture,
    /// Current resolution of the frontend's framebuffer
    frontend_resolution: (u32, u32),
    /// Current internal resolution upscaling factor
    internal_upscaling: u32,
    /// Current internal color depth
    internal_color_depth: u8,
}

impl GlState {
    pub fn from_config(config: DrawConfig) -> Result<GlState, Error> {
        let upscaling = CoreVariables::internal_upscale_factor();
        let depth = CoreVariables::internal_color_depth();
        let scale_dither = CoreVariables::scale_dither();
        let wireframe = CoreVariables::wireframe();

        info!("Building OpenGL state ({}x internal res., {}bpp)",
              upscaling, depth);

        let command_buffer =
            try!(GlState::build_buffer(
                include_str!("shaders/command_vertex.glsl"),
                include_str!("shaders/command_fragment.glsl"),
                2048));

        let output_buffer =
            try!(GlState::build_buffer(
                include_str!("shaders/output_vertex.glsl"),
                include_str!("shaders/output_fragment.glsl"),
                4));

        let image_load_buffer =
            try!(GlState::build_buffer(
                include_str!("shaders/image_load_vertex.glsl"),
                include_str!("shaders/image_load_fragment.glsl"),
                4));

        let native_width = VRAM_WIDTH_PIXELS as u32;
        let native_height = VRAM_HEIGHT as u32;

        // Integer texture holding the raw VRAM texture contents. We
        // can't meaningfully upscale it since most games use paletted
        // textures.
        let fb_texture =
            try!(Texture::new(native_width, native_height, gl::R16UI));

        if depth > 16 {
            // Dithering is superfluous when we increase the internal
            // color depth
            try!(command_buffer.disable_attribute("dither"));
        }

        let dither_scaling =
            if scale_dither {
                upscaling
            } else {
                1
            };

        let command_draw_mode =
            if wireframe {
                gl::LINE
            } else {
                gl::FILL
            };

        try!(command_buffer.program()
             .uniform1ui("dither_scaling", dither_scaling));

        let texture_storage =
            match depth {
                16 => gl::RGB5_A1,
                32 => gl::RGBA8,
                _ => panic!("Unsupported depth {}", depth),
            };

        let fb_out = try!(Texture::new(native_width * upscaling,
                                       native_height * upscaling,
                                       texture_storage));

        let mut state = GlState {
            command_buffer: command_buffer,
            command_draw_mode: gl::TRIANGLES,
            command_polygon_mode: command_draw_mode,
            output_buffer: output_buffer,
            image_load_buffer: image_load_buffer,
            config: config,
            fb_texture: fb_texture,
            fb_out: fb_out,
            frontend_resolution: (0, 0),
            internal_upscaling: upscaling,
            internal_color_depth: depth,
        };

        let vram_contents = *state.config.vram;

        // Load the VRAM contents into the textures
        try!(state.upload_textures((0, 0),
                                   (VRAM_WIDTH_PIXELS, VRAM_HEIGHT),
                                   &vram_contents));

        Ok(state)
    }

    fn build_buffer<T>(vertex_shader: &str,
                       fragment_shader: &str,
                       capacity: usize) -> Result<DrawBuffer<T>, Error>
        where T: ::retrogl::vertex::Vertex {

        let vs = try!(Shader::new(vertex_shader, ShaderType::Vertex));

        let fs = try!(Shader::new(fragment_shader, ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        DrawBuffer::new(capacity, program)
    }

    fn draw(&mut self) -> Result<(), Error> {

        if self.command_buffer.empty() {
            // Nothing to be done
            return Ok(())
        }

        unsafe {
            // XXX No semi-transparency support for now
            gl::BlendFuncSeparate(gl::ONE,
                                  gl::ZERO,
                                  gl::ONE,
                                  gl::ZERO);
            gl::Enable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
            gl::PolygonMode(gl::FRONT_AND_BACK, self.command_polygon_mode);
        }

        let (x, y) = self.config.draw_offset;

        try!(self.command_buffer.program().uniform2i("offset",
                                                     x as GLint,
                                                     y as GLint));

        // We use texture unit 0
        try!(self.command_buffer.program().uniform1i("fb_texture", 0));

        // Bind the out framebuffer
        let _fb = Framebuffer::new(&self.fb_out);

        try!(self.command_buffer.draw(self.command_draw_mode));

        unsafe {
            gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
        }

        self.command_buffer.clear()
    }

    fn apply_scissor(&mut self) {
        let (x, y) = self.config.draw_area_top_left;
        let (w, h) = self.config.draw_area_dimensions;

        let upscale = self.internal_upscaling as GLsizei;

        // We need to scale those to match the internal resolution if
        // upscaling is enabled
        let x = (x as GLsizei) * upscale;
        let y = (y as GLsizei) * upscale;
        let w = (w as GLsizei) * upscale;
        let h = (h as GLsizei) * upscale;

        unsafe {
            gl::Scissor(x, y, w, h);
        }
    }

    fn bind_libretro_framebuffer(&mut self) {
        let (f_w, f_h) = self.frontend_resolution;
        let (w, h) = self.config.display_resolution;

        let upscale = self.internal_upscaling;

        // XXX scale w and h when implementing increased internal
        // resolution
        let w = (w as u32) * upscale;
        let h = (h as u32) * upscale;

        if w != f_w || h != f_h {
            // We need to change the frontend's resolution
            let geometry = libretro::GameGeometry {
                base_width: w as c_uint,
                base_height: h as c_uint,
                // Max parameters are ignored by this call
                max_width: 0,
                max_height: 0,
                // Is this accurate?
                aspect_ratio: 4./3.,
            };

            info!("Target framebuffer size: {}x{}", w, h);

            libretro::set_geometry(&geometry);

            self.frontend_resolution = (w, h);
        }

        // Bind the output framebuffer provided by the frontend
        let fbo = libretro::hw_context::get_current_framebuffer() as GLuint;

        unsafe {
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, fbo);
            gl::Viewport(0, 0, w as GLsizei, h as GLsizei);
        }
    }

    fn upload_textures(&mut self,
                       top_left: (u16, u16),
                       dimensions: (u16, u16),
                       pixel_buffer: &[u16]) -> Result<(), Error> {

        try!(self.fb_texture.set_sub_image(top_left,
                                           dimensions,
                                           gl::RED_INTEGER,
                                           gl::UNSIGNED_SHORT,
                                           pixel_buffer));

        try!(self.image_load_buffer.clear());

        let x_start = top_left.0;
        let x_end = x_start + dimensions.0;
        let y_start = top_left.1;
        let y_end = y_start + dimensions.1;

        try!(self.image_load_buffer.push_slice(
            &[ImageLoadVertex { position: [x_start, y_start] },
              ImageLoadVertex { position: [x_end, y_start] },
              ImageLoadVertex { position: [x_start, y_end] },
              ImageLoadVertex { position: [x_end, y_end] },
              ]));

        try!(self.image_load_buffer.program().uniform1i("fb_texture", 0));

        unsafe {
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
        }

        // Bind the output framebuffer
        let _fb = Framebuffer::new(&self.fb_out);

        self.image_load_buffer.draw(gl::TRIANGLE_STRIP)
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

        self.apply_scissor();

        // In case we're upscaling we need to increase the line width
        // proportionally
        unsafe {
            gl::LineWidth(self.internal_upscaling as GLfloat);
        }

        // Bind `fb_texture` to texture unit 0
        self.fb_texture.bind(gl::TEXTURE0);
    }

    fn refresh_variables(&mut self) -> bool {
        let upscaling = CoreVariables::internal_upscale_factor();
        let depth = CoreVariables::internal_color_depth();
        let scale_dither = CoreVariables::scale_dither();
        let wireframe = CoreVariables::wireframe();

        let rebuild_fb_out =
            upscaling != self.internal_upscaling ||
            depth != self.internal_color_depth;

        if rebuild_fb_out {

            if depth > 16 {
                self.command_buffer.disable_attribute("dither").unwrap()
            } else {
                self.command_buffer.enable_attribute("dither").unwrap()
            }

            let native_width = VRAM_WIDTH_PIXELS as u32;
            let native_height = VRAM_HEIGHT as u32;

            let w = native_width * upscaling;
            let h = native_height * upscaling;

            let texture_storage =
                match depth {
                    16 => gl::RGB5_A1,
                    32 => gl::RGBA8,
                    _ => panic!("Unsupported depth {}", depth),
                };

            let fb_out = Texture::new(w, h, texture_storage).unwrap();

            self.fb_out = fb_out;

            let vram_contents = *self.config.vram;

            // This is a bit wasteful since it'll re-upload the data
            // to `fb_texture` even though we haven't touched it but
            // this code is not very performance-critical anyway.
            self.upload_textures((0, 0),
                                 (VRAM_WIDTH_PIXELS, VRAM_HEIGHT),
                                 &vram_contents).unwrap();
        }

        let dither_scaling =
            if scale_dither {
                upscaling
            } else {
                1
            };

        self.command_buffer.program()
            .uniform1ui("dither_scaling", dither_scaling).unwrap();

        self.command_polygon_mode =
            if wireframe {
                gl::LINE
            } else {
                gl::FILL
            };

        unsafe {
            gl::LineWidth(upscaling as GLfloat);
        }

        // If the scaling factor has changed the frontend should be
        // reconfigured. We can't do that here because it could
        // destroy the OpenGL context which would destroy `self`
        let reconfigure_frontend = self.internal_upscaling != upscaling;

        self.internal_upscaling = upscaling;
        self.internal_color_depth = depth;

        return reconfigure_frontend
    }

    fn finalize_frame(&mut self) {
        // Draw pending commands
        self.draw().unwrap();

        // We can now render to the frontend's buffer.
        self.bind_libretro_framebuffer();

        unsafe {
            gl::ClearColor(1., 0., 0., 0.);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

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

        let depth_24bpp = self.config.display_24bpp as GLint;

        self.output_buffer.program()
            .uniform1i("fb", 1).unwrap();
        self.output_buffer.program()
            .uniform1i("depth_24bpp", depth_24bpp).unwrap();

        self.output_buffer.draw(gl::TRIANGLE_STRIP).unwrap();

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
            gl::LineWidth(1.);
        }

        libretro::gl_frame_done(self.frontend_resolution.0,
                                self.frontend_resolution.1)
    }
}

impl Renderer for GlState {
    fn set_draw_offset(&mut self, x: i16, y: i16) {
        self.config.draw_offset = (x, y)
    }

    fn set_draw_area(&mut self, top_left: (u16, u16), dimensions: (u16, u16)) {
        // Finish drawing anything in the current area
        self.draw().unwrap();

        self.config.draw_area_top_left = top_left;
        self.config.draw_area_dimensions = dimensions;

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
                 attributes: &PrimitiveAttributes,
                 vertices: &[Vertex; 2]) {

        let force_draw =
            self.command_buffer.remaining_capacity() < 2 ||
            self.command_draw_mode != gl::LINES;

        if force_draw {
            self.draw().unwrap();
            self.command_draw_mode = gl::LINES;
        }

        let v: ArrayVec<[_; 2]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(attributes, v))
            .collect();

        self.command_buffer.push_slice(&v).unwrap();
    }

    fn push_triangle(&mut self,
                     attributes: &PrimitiveAttributes,
                     vertices: &[Vertex; 3]) {

        let force_draw =
            self.command_buffer.remaining_capacity() < 3 ||
            self.command_draw_mode != gl::TRIANGLES;

        if force_draw {
            self.draw().unwrap();
            self.command_draw_mode = gl::TRIANGLES;
        }

        let v: ArrayVec<[_; 3]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(attributes, v))
            .collect();

        self.command_buffer.push_slice(&v).unwrap();
    }

    fn push_quad(&mut self,
                 attributes: &PrimitiveAttributes,
                 vertices: &[Vertex; 4]) {
        let force_draw =
            self.command_buffer.remaining_capacity() < 6 ||
            self.command_draw_mode != gl::TRIANGLES;

        if force_draw {
            self.draw().unwrap();
            self.command_draw_mode = gl::TRIANGLES;
        }

        let v: ArrayVec<[_; 4]> =
            vertices.iter().map(|v| CommandVertex::from_vertex(attributes, v))
            .collect();

        self.command_buffer.push_slice(&v[0..3]).unwrap();
        self.command_buffer.push_slice(&v[1..4]).unwrap();
    }

    fn fill_rect(&mut self,
                 color: [u8; 3],
                 top_left: (u16, u16),
                 dimensions: (u16, u16)) {
        // Draw pending commands
        self.draw().unwrap();

        let x_start = top_left.0;
        let x_end = x_start + dimensions.0;
        let y_start = top_left.1;
        let y_end = y_start + dimensions.1;

        // We reuse the normal command processing program since it's
        // pretty much equivalent to drawing a non-textured polygon
        let v: ArrayVec<[_; 4]> = [(x_start, y_start),
                                   (x_end, y_start),
                                   (x_start, y_end),
                                   (x_end, y_end)]
            .iter()
            .map(|&(x, y)| {
                CommandVertex {
                    position: [x as i16, y as i16],
                    color: color,
                    // No texture
                    texture_blend_mode: 0,
                    ..Default::default()
                }
            })
            .collect();

        self.command_buffer.push_slice(&v).unwrap();

        // We use texture unit 0
        self.command_buffer.program().uniform1i("fb_texture", 0).unwrap();

        // Bind the out framebuffer
        let _fb = Framebuffer::new(&self.fb_out);

        // Fill rect commands don't use the draw offset
        self.command_buffer.program().uniform2i("offset", 0, 0).unwrap();

        unsafe {
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::BLEND);
        }

        self.command_buffer.draw(gl::TRIANGLE_STRIP).unwrap();

        self.command_buffer.clear().unwrap();
    }

    fn load_image(&mut self,
                  top_left: (u16, u16),
                  resolution: (u16, u16),
                  pixel_buffer: &[u16]) {
        self.draw().unwrap();

        let x_start = top_left.0 as usize;
        let y_start = top_left.1 as usize;

        let w = resolution.0 as usize;
        let h = resolution.1 as usize;

        // Update the VRAM buffer (this way we won't lose the textures
        // if the GL context gets destroyed)
        match Rc::get_mut(&mut self.config.vram) {
            Some(vram) =>
                for y in 0..h {
                    for x in 0..w {
                        let fb_x = x_start + x;
                        let fb_y = y_start + y;

                        let fb_w = VRAM_WIDTH_PIXELS as usize;

                        let fb_index = fb_y * fb_w + fb_x;
                        let buffer_index = y * w + x;

                        vram[fb_index] = pixel_buffer[buffer_index];
                    }
                },
            None => panic!("VRAM is shared!"),
        }

        self.upload_textures(top_left, resolution, pixel_buffer).unwrap();
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

struct ImageLoadVertex {
    /// Vertex position in VRAM
    position: [u16; 2],
}

implement_vertex!(ImageLoadVertex,
                  position);
