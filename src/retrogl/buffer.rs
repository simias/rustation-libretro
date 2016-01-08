use std::marker::PhantomData;
use std::mem::size_of;
use std::ptr;

use gl;
use gl::types::{GLint, GLuint, GLsizeiptr, GLintptr, GLsizei};

use retrogl::error::{Error, error_or, get_error};
use retrogl::vertex::VertexArrayObject;
use retrogl::program::Program;

pub struct DrawBuffer<T> {
    /// Vertex Array Object containing the bindings for this
    /// buffer. I'm assuming that each VAO will only use a single
    /// buffer for simplicity.
    vao: VertexArrayObject,
    /// Program used to draw this buffer
    program: Program,
    /// Number of elements T that the vertex buffer can hold
    capacity: usize,
    /// OpenGL name for this buffer
    id: GLuint,
    /// Marker for the type of our buffer's contents
    contains: PhantomData<T>,
    /// Current number of entries in the buffer
    len: usize,
}

impl<T> DrawBuffer<T> {

    pub fn new(capacity: usize,
               program: Program) -> Result<DrawBuffer<T>, Error> {

        let vao = try!(VertexArrayObject::new());

        let mut id = 0;

        unsafe {
            // Generate the buffer object
            gl::GenBuffers(1, &mut id);
        };

        let mut buf = DrawBuffer {
            vao: vao,
            program: program,
            capacity: capacity,
            id: id,
            contains: PhantomData::<T>,
            len: 0,
        };

        try!(buf.clear());

        try!(buf.bind_attributes());

        error_or(buf)
    }

    /// Specify the vertex attriute layout and bind them to the VAO
    fn bind_attributes(&self)-> Result<(), Error> {
        self.vao.bind();

        // ARRAY_BUFFER is captured by VertexAttribPointer
        self.bind();

        let attr = try!(self.program.find_attribute("coords"));

        unsafe { gl::EnableVertexAttribArray(attr) };

        // This captures the buffer so that we don't have to bind it
        // when we draw later on, we'll just have to bind the vao.
        unsafe {
            gl::VertexAttribIPointer(attr,
                                     2,
                                     gl::SHORT,
                                     size_of::<T>() as GLint,
                                     0 as *const _)
        }

        let attr = try!(self.program.find_attribute("color"));

        unsafe { gl::EnableVertexAttribArray(attr) };

        // This captures the buffer so that we don't have to bind it
        // when we draw later on, we'll just have to bind the vao.
        unsafe {
            gl::VertexAttribIPointer(attr,
                                     3,
                                     gl::BYTE,
                                     size_of::<T>() as GLint,
                                     4 as *const _)
        }

        get_error()
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Orphan the buffer (to avoid synchronization) and allocate a
    /// new one.
    ///
    /// https://www.opengl.org/wiki/Buffer_Object_Streaming
    pub fn clear(&mut self) -> Result<(), Error> {
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

        self.len = 0;

        get_error()
    }

    /// Bind the buffer to the current VAO
    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.id);
        }
    }

    pub fn push_slice(&mut self,
                      slice: &[T]) -> Result<(), Error> {
        let n = slice.len();

        if n > self.remaining_capacity() {
            return Err(Error::OutOfMemory);
        }

        let element_size = size_of::<T>();

        let offset_bytes = self.len * element_size;

        let size_bytes = n * element_size;

        self.bind();

        unsafe {
            gl::BufferSubData(gl::ARRAY_BUFFER,
                              offset_bytes as GLintptr,
                              size_bytes as GLintptr,
                              slice.as_ptr() as *const _);
        }

        try!(get_error());

        self.len += n;

        Ok(())
    }

    pub fn draw_triangles(&mut self) -> Result<(), Error> {
        self.vao.bind();
        self.program.bind();

        unsafe { gl::DrawArrays(gl::TRIANGLES, 0, self.len as GLsizei) };

        get_error()
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity - self.len
    }
}

impl<T> Drop for DrawBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.id);
        }
    }
}
