use gl;
use gl::types::{GLint, GLuint, GLenum, GLsizei};

use retrogl::error::{Error, error_or, get_error};

pub struct Texture {
    id: GLuint,
}

impl Texture {
    pub fn new(width: u32,
               height: u32,
               internal_format: GLenum) -> Result<Texture, Error> {
        let mut id = 0;

        unsafe {
            gl::GenTextures(1, &mut id);
            gl::BindTexture(gl::TEXTURE_2D, id);
            gl::TexStorage2D(gl::TEXTURE_2D,
                             1,
                             internal_format,
                             width as GLsizei,
                             height as GLsizei);
        }

        error_or(Texture {
            id: id,
        })
    }

    pub fn bind(&self, texture_unit: GLenum) {
        unsafe {
            gl::ActiveTexture(texture_unit);
            gl::BindTexture(gl::TEXTURE_2D, self.id);
        }
    }

    pub fn set_sub_image<T>(&self,
                            top_left: (u16, u16),
                            resolution: (u16, u16),
                            format: GLenum,
                            ty: GLenum,
                            data: &[T]) -> Result<(), Error> {

        if data.len() != (resolution.0 as usize * resolution.1 as usize) {
            panic!("Invalid texture sub_image size");
        }

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.id);
            gl::TexSubImage2D(gl::TEXTURE_2D,
                              0,
                              top_left.0 as GLint,
                              top_left.1 as GLint,
                              resolution.0 as GLsizei,
                              resolution.1 as GLsizei,
                              format,
                              ty,
                              data.as_ptr() as *const _);
        }

        get_error()
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.id);
        }
    }
}
