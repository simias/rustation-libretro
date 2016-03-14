use gl;
use gl::types::{GLint, GLuint, GLenum, GLsizei};

use retrogl::error::{Error, error_or, get_error};

pub struct Texture {
    id: GLuint,
    width: u32,
    height: u32,
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
            width: width,
            height: height,
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

        // if data.len() != (resolution.0 as usize * resolution.1 as usize) {
        //     panic!("Invalid texture sub_image size");
        // }

        unsafe {
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
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

    pub fn set_sub_image_window<T>(&self,
                                   top_left: (u16, u16),
                                   resolution: (u16, u16),
                                   row_len: usize,
                                   format: GLenum,
                                   ty: GLenum,
                                   data: &[T]) -> Result<(), Error> {

        let (x, y) = top_left;

        let index = (y as usize) * row_len + (x as usize);

        let data = &data[index..];

        unsafe {
            gl::PixelStorei(gl::UNPACK_ROW_LENGTH, row_len as GLint);
        }

        let r = self.set_sub_image(top_left, resolution, format, ty, data);

        unsafe {
            gl::PixelStorei(gl::UNPACK_ROW_LENGTH, 0);
        }

        r
    }

    pub unsafe fn id(&self) -> GLuint {
        self.id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.id);
        }
    }
}
