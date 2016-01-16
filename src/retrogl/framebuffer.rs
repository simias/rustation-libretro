use gl;
use gl::types::{GLuint, GLsizei};

use retrogl::error::{Error, error_or};
use retrogl::texture::Texture;

pub struct Framebuffer<'a> {
    id: GLuint,
    _color_texture: &'a Texture,
}

impl<'a> Framebuffer<'a> {
    pub fn new<'n>(color_texture: &'n Texture)
                   -> Result<Framebuffer<'n>, Error> {

        let mut id = 0;

        unsafe {
            gl::GenFramebuffers(1, &mut id);
        }

        let fb = Framebuffer {
            id: id,
            _color_texture: color_texture,
        };

        fb.bind();

        unsafe {
            gl::FramebufferTexture(gl::DRAW_FRAMEBUFFER,
                                   gl::COLOR_ATTACHMENT0,
                                   color_texture.id(),
                                   0);

            gl::DrawBuffers(1, &gl::COLOR_ATTACHMENT0);
            gl::Viewport(0,
                         0,
                         color_texture.width() as GLsizei,
                         color_texture.height() as GLsizei);
        }

        error_or(fb)
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, self.id);
        }
    }
}

impl<'a> Drop for Framebuffer<'a> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.id);
        }
    }
}
