use std::marker::PhantomData;
use std::mem::size_of;
use std::ptr;

use gl;
use gl::types::{GLuint, GLsizeiptr};

use retrogl::error::{Error, error_or};

pub struct VertexBuffer<T> {
    /// Number of elements T that the vertex buffer can hold
    capacity: usize,
    /// OpenGL name for this buffer
    id: GLuint,
    /// Marker for the type of our buffer's contents
    contains: PhantomData<T>,
}

impl<T> VertexBuffer<T> {

    pub fn new(capacity: usize) -> Result<VertexBuffer<T>, Error> {
        let mut id = 0;

        unsafe {
            // Generate the buffer object
            gl::GenBuffers(1, &mut id);
        };

        let buf = VertexBuffer {
            capacity: capacity,
            id: id,
            contains: PhantomData::<T>,
        };

        buf.clear();

        error_or(buf)
    }

    /// Orphan the buffer (to avoid synchronization) and allocate a
    /// new one.
    ///
    /// https://www.opengl.org/wiki/Buffer_Object_Streaming
    pub fn clear(&self) {
        self.bind();

        unsafe {
            // Compute the size of the buffer
            let element_size = size_of::<T>();

            let storage_size = (self.capacity * element_size) as GLsizeiptr;

            gl::BufferData(gl::ARRAY_BUFFER,
                           storage_size,
                           ptr::null(),
                           gl::DYNAMIC_DRAW);
        }
    }

    /// Bind the buffer to the current VAO
    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.id);
        }
    }
}

impl<T> Drop for VertexBuffer<T> {
    fn drop(&mut self) {
        self.bind();
        unsafe {
            gl::DeleteBuffers(1, &self.id);
        }
    }
}
