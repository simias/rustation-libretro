use gl;
use gl::types::GLuint;

use retrogl::error::{Error, error_or};

pub struct VertexArrayObject {
    id: GLuint,
}

impl VertexArrayObject {
    pub fn new() -> Result<VertexArrayObject, Error> {
        let mut id = 0;

        unsafe {
            gl::GenVertexArrays(1, &mut id);
        }

        error_or(VertexArrayObject {
            id: id,
        })
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindVertexArray(self.id);
        }
    }
}

impl Drop for VertexArrayObject {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.id);
        }
    }
}
