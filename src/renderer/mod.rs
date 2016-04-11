use gl;
use gl::types::{GLuint, GLint, GLsizei, GLenum, GLfloat};
use arrayvec::ArrayVec;
use libc::c_uint;
use rustation::gpu::renderer::{Renderer, Vertex, PrimitiveAttributes};
use rustation::gpu::renderer::{TextureDepth, BlendMode, SemiTransparencyMode};
use rustation::gpu::{VRAM_WIDTH_PIXELS, VRAM_HEIGHT};

use retrogl::DrawConfig;
use retrogl::error::{Error, get_error};
use retrogl::buffer::DrawBuffer;
use retrogl::shader::{Shader, ShaderType};
use retrogl::program::Program;
use retrogl::types::GlType;
use retrogl::texture::Texture;
use retrogl::framebuffer::Framebuffer;

use CoreVariables;

use libretro;

pub struct GlRenderer {
    /// Buffer used to handle PlayStation GPU draw commands
    command_buffer: DrawBuffer<CommandVertex>,
    /// Primitive type for the vertices in the command buffers
    /// (TRIANGLES or LINES)
    command_draw_mode: GLenum,
    /// Temporary buffer holding vertices for semi-transparent draw
    /// commands.
    semi_transparent_vertices: Vec<CommandVertex>,
    /// Transparency mode for semi-transparent commands
    semi_transparency_mode: SemiTransparencyMode,
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
    /// Depth buffer for fb_out
    fb_out_depth: Texture,
    /// Current resolution of the frontend's framebuffer
    frontend_resolution: (u32, u32),
    /// Current internal resolution upscaling factor
    internal_upscaling: u32,
    /// Current internal color depth
    internal_color_depth: u8,
    /// Counter for preserving primitive draw order in the z-buffer
    /// since we draw semi-transparent primitives out-of-order.
    primitive_ordering: i16,
}

impl GlRenderer {
    pub fn from_config(config: DrawConfig) -> Result<GlRenderer, Error> {

        let upscaling = CoreVariables::internal_upscale_factor();
        let depth = CoreVariables::internal_color_depth();
        let scale_dither = CoreVariables::scale_dither();
        let wireframe = CoreVariables::wireframe();

        info!("Building OpenGL state ({}x internal res., {}bpp)",
              upscaling, depth);

        let opaque_command_buffer =
            try!(GlRenderer::build_buffer(
                include_str!("shaders/command_vertex.glsl"),
                include_str!("shaders/command_fragment.glsl"),
                2048,
                true));

        let output_buffer =
            try!(GlRenderer::build_buffer(
                include_str!("shaders/output_vertex.glsl"),
                include_str!("shaders/output_fragment.glsl"),
                4,
                false));

        let image_load_buffer =
            try!(GlRenderer::build_buffer(
                include_str!("shaders/image_load_vertex.glsl"),
                include_str!("shaders/image_load_fragment.glsl"),
                4,
                false));

        let native_width = VRAM_WIDTH_PIXELS as u32;
        let native_height = VRAM_HEIGHT as u32;

        // Texture holding the raw VRAM texture contents. We can't
        // meaningfully upscale it since most games use paletted
        // textures.
        let fb_texture =
            try!(Texture::new(native_width, native_height, gl::RGB5_A1));

        if depth > 16 {
            // Dithering is superfluous when we increase the internal
            // color depth
            try!(opaque_command_buffer.disable_attribute("dither"));
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

        try!(opaque_command_buffer.program()
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

        let fb_out_depth = try!(Texture::new(fb_out.width(),
                                             fb_out.height(),
                                             gl::DEPTH_COMPONENT32F));

        let mut state = GlRenderer {
            command_buffer: opaque_command_buffer,
            command_draw_mode: gl::TRIANGLES,
            semi_transparent_vertices: Vec::with_capacity(2048),
            semi_transparency_mode: SemiTransparencyMode::Average,
            command_polygon_mode: command_draw_mode,
            output_buffer: output_buffer,
            image_load_buffer: image_load_buffer,
            config: config,
            fb_texture: fb_texture,
            fb_out: fb_out,
            fb_out_depth: fb_out_depth,
            frontend_resolution: (0, 0),
            internal_upscaling: upscaling,
            internal_color_depth: depth,
            primitive_ordering: 0,
        };

        // Yet an other copy of this 1MB array to make the borrow
        // checker happy...
        let vram_contents = state.config.vram.clone();

        // Load the VRAM contents into the textures
        try!(state.upload_textures((0, 0),
                                   (VRAM_WIDTH_PIXELS, VRAM_HEIGHT),
                                   &vram_contents));

        Ok(state)
    }

    fn build_buffer<T>(vertex_shader: &str,
                       fragment_shader: &str,
                       capacity: usize,
                       lifo: bool) -> Result<DrawBuffer<T>, Error>
        where T: ::retrogl::vertex::Vertex {

        let vs = try!(Shader::new(vertex_shader, ShaderType::Vertex));

        let fs = try!(Shader::new(fragment_shader, ShaderType::Fragment));

        let program = try!(Program::new(vs, fs));

        DrawBuffer::new(capacity, program, lifo)
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
            gl::Disable(gl::BLEND);
        }

        let (x, y) = self.config.draw_offset;

        try!(self.command_buffer.program().uniform2i("offset",
                                                     x as GLint,
                                                     y as GLint));

        // We use texture unit 0
        try!(self.command_buffer.program().uniform1i("fb_texture", 0));

        try!(self.command_buffer.program()
             .uniform1ui("draw_semi_transparent", 0));

        // Bind the out framebuffer
        let _fb = Framebuffer::new_with_depth(&self.fb_out, &self.fb_out_depth);

        unsafe {
            gl::Clear(gl::DEPTH_BUFFER_BIT);
        }

        try!(self.command_buffer.draw(self.command_draw_mode));

        self.primitive_ordering = 0;

        self.semi_transparent_vertices.clear();
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
                                           gl::RGBA,
                                           gl::UNSIGNED_SHORT_1_5_5_5_REV,
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
            gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
        }

        // Bind the output framebuffer
        let _fb = Framebuffer::new(&self.fb_out);

        try!(self.image_load_buffer.draw(gl::TRIANGLE_STRIP));

        unsafe {
            gl::PolygonMode(gl::FRONT_AND_BACK, self.command_polygon_mode);
            gl::Enable(gl::SCISSOR_TEST);
        }

        get_error()
    }

    pub fn draw_config(&self) -> &DrawConfig {
        &self.config
    }

    pub fn prepare_render(&mut self) {

        self.apply_scissor();

        // In case we're upscaling we need to increase the line width
        // proportionally
        unsafe {
            gl::LineWidth(self.internal_upscaling as GLfloat);
            gl::PolygonMode(gl::FRONT_AND_BACK, self.command_polygon_mode);
            gl::Enable(gl::SCISSOR_TEST);
            gl::Enable(gl::DEPTH_TEST);
            gl::DepthFunc(gl::LEQUAL);
        }

        // Bind `fb_texture` to texture unit 0
        self.fb_texture.bind(gl::TEXTURE0);
    }

    pub fn refresh_variables(&mut self) -> bool {
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

            let vram_contents = self.config.vram.clone();

            // This is a bit wasteful since it'll re-upload the data
            // to `fb_texture` even though we haven't touched it but
            // this code is not very performance-critical anyway.
            self.upload_textures((0, 0),
                                 (VRAM_WIDTH_PIXELS, VRAM_HEIGHT),
                                 &*vram_contents).unwrap();

            self.fb_out_depth =
                Texture::new(w, h, gl::DEPTH_COMPONENT32F).unwrap();
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

    pub fn finalize_frame(&mut self) {
        // Draw pending commands
        self.draw().unwrap();

        // We can now render to the frontend's buffer.
        self.bind_libretro_framebuffer();

        // Bind `fb_out` to texture unit 1
        self.fb_out.bind(gl::TEXTURE1);

        // First we draw the visible part of fb_out
        unsafe {
            gl::Disable(gl::SCISSOR_TEST);
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::BLEND);
            gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
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
        self.output_buffer.program()
            .uniform1ui("internal_upscaling", self.internal_upscaling).unwrap();

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

    /// Check if a new primitive's attributes are somehow incompatible
    /// with the ones currently buffered, in which case we must force
    /// a draw to flush the buffers.
    fn maybe_force_draw(&mut self,
                        nvertices: usize,
                        draw_mode: GLenum,
                        attributes: &PrimitiveAttributes) {
        let force_draw =
            // Check if we have enough room left in the buffer
            self.command_buffer.remaining_capacity() < nvertices ||
            // Check if we're changing the draw mode (line <=> triangle)
            self.command_draw_mode != draw_mode ||
            // Check if we're changing the semi-transparency mode
            (attributes.semi_transparent &&
             !self.semi_transparent_vertices.is_empty() &&
             self.semi_transparency_mode != attributes.semi_transparency_mode);

        if force_draw {
            self.draw().unwrap();

            // Update the state machine for the next primitive
            self.command_draw_mode = draw_mode;

            if attributes.semi_transparent {
                self.semi_transparency_mode = attributes.semi_transparency_mode;
            }
        }
    }
}

impl Renderer for GlRenderer {
    fn set_draw_offset(&mut self, x: i16, y: i16) {
        // Finish drawing anything with the current offset
        self.draw().unwrap();
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

        self.maybe_force_draw(2, gl::LINES, attributes);

        let z = self.primitive_ordering;

        self.primitive_ordering += 1;

        let iter =
            vertices.iter().map(|v|
                                CommandVertex::from_vertex(attributes, v, z));

        if attributes.semi_transparent {
            self.semi_transparent_vertices.extend(iter);
        } else {
            let v: ArrayVec<[_; 2]> = iter.collect();

            self.command_buffer.push_slice(&v).unwrap();
        }
    }

    fn push_triangle(&mut self,
                     attributes: &PrimitiveAttributes,
                     vertices: &[Vertex; 3]) {

        self.maybe_force_draw(3, gl::TRIANGLES, attributes);

        let z = self.primitive_ordering;

        self.primitive_ordering += 1;

        let v: ArrayVec<[_; 3]> =
            vertices.iter().map(|v|
                                CommandVertex::from_vertex(attributes, v, z))
            .collect();

        let needs_opaque_draw =
            !attributes.semi_transparent ||
            // Textured semi-transparent polys can contain opaque
            // texels (when bit 15 of the color is set to
            // 0). Therefore they're drawn twice, once for the opaque
            // texels and once for the semi-transparent ones
            attributes.blend_mode != BlendMode::None;

        if needs_opaque_draw {
            self.command_buffer.push_slice(&v).unwrap();
        }

        if attributes.semi_transparent {
            self.semi_transparent_vertices.extend_from_slice(&v);
        }
    }

    fn push_quad(&mut self,
                 attributes: &PrimitiveAttributes,
                 vertices: &[Vertex; 4]) {

        self.maybe_force_draw(6, gl::TRIANGLES, attributes);

        let z = self.primitive_ordering;

        self.primitive_ordering += 1;

        let v: ArrayVec<[_; 4]> =
            vertices.iter().map(|v|
                                CommandVertex::from_vertex(attributes, v, z))
            .collect();

        let needs_opaque_draw =
            !attributes.semi_transparent ||
            // Textured semi-transparent polys can contain opaque
            // texels (when bit 15 of the color is set to
            // 0). Therefore they're drawn twice, once for the opaque
            // texels and once for the semi-transparent ones
            attributes.blend_mode != BlendMode::None;

        if needs_opaque_draw {
            self.command_buffer.push_slice(&v[0..3]).unwrap();
            self.command_buffer.push_slice(&v[1..4]).unwrap();
        }

        if attributes.semi_transparent {
            self.semi_transparent_vertices.extend_from_slice(&v[0..3]);
            self.semi_transparent_vertices.extend_from_slice(&v[1..4]);
        }
    }

    fn fill_rect(&mut self,
                 color: [u8; 3],
                 top_left: (u16, u16),
                 dimensions: (u16, u16)) {
        // Draw pending commands
        self.draw().unwrap();

        // Fill rect ignores the draw area. Save the previous scissor
        // settings and reconfigure the scissor box to the fill
        // rectangle insteadd.
        let draw_area_top_left = self.config.draw_area_top_left;
        let draw_area_dimensions = self.config.draw_area_dimensions;

        self.config.draw_area_top_left = top_left;
        self.config.draw_area_dimensions = dimensions;

        self.apply_scissor();

        // ClearColor takes normalized floating point color components
        let clear_color: ArrayVec<[_; 3]> =
            color.iter().map(|&c| (c as f32) / 255.)
            .collect();

        {
            // Bind the out framebuffer
            let _fb = Framebuffer::new(&self.fb_out);

            unsafe {
                gl::ClearColor(clear_color[0],
                               clear_color[1],
                               clear_color[2],
                               // XXX Not entirely sure what happens
                               // to the mask bit in fill_rect. No$
                               // seems to say that it's set to 0.
                               0.);
                gl::Clear(gl::COLOR_BUFFER_BIT);
            }
        }

        // Reconfigure the draw area
        self.config.draw_area_top_left = draw_area_top_left;
        self.config.draw_area_dimensions = draw_area_dimensions;

        self.apply_scissor();
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
        for y in 0..h {
            for x in 0..w {
                let fb_x = x_start + x;
                let fb_y = y_start + y;

                let fb_w = VRAM_WIDTH_PIXELS as usize;

                let fb_index = fb_y * fb_w + fb_x;
                let buffer_index = y * w + x;

                self.config.vram[fb_index] = pixel_buffer[buffer_index];
            }
        }

        self.upload_textures(top_left, resolution, pixel_buffer).unwrap();
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct CommandVertex {
    /// Position in PlayStation VRAM coordinates
    position: [i16; 3],
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
    /// 0: primitive is opaque, 1: primitive is semi-transparent
    semi_transparent: u8,
}

implement_vertex!(CommandVertex,
                  position, color, texture_page,
                  texture_coord, clut, texture_blend_mode,
                  depth_shift, dither, semi_transparent);

impl CommandVertex {
    fn from_vertex(attributes: &PrimitiveAttributes,
                   v: &Vertex,
                   z: i16) -> CommandVertex {
        CommandVertex {
            position: [v.position[0], v.position[1], z],
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
            semi_transparent: attributes.semi_transparent as u8,
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
