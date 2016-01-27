use std::marker::PhantomData;
use std::mem::size_of;
use std::ptr;

use gl;
use gl::types::{GLint, GLuint, GLsizeiptr, GLintptr, GLsizei, GLenum};

use retrogl::error::{Error, error_or, get_error};
use retrogl::vertex::{Vertex, VertexArrayObject};
use retrogl::program::Program;
use retrogl::types::Kind;

pub struct DrawBuffer<T> {
    /// OpenGL name for this buffer
    id: GLuint,
    /// Vertex Array Object containing the bindings for this
    /// buffer. I'm assuming that each VAO will only use a single
    /// buffer for simplicity.
    vao: VertexArrayObject,
    /// Program used to draw this buffer
    program: Program,
    /// Number of elements T that the vertex buffer can hold
    capacity: usize,
    /// Marker for the type of our buffer's contents
    contains: PhantomData<T>,
    /// Current number of entries in the buffer
    len: usize,
}

impl<T: Vertex> DrawBuffer<T> {

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

        let attributes = T::attributes();

        let element_size = size_of::<T>() as GLint;

        for attr in attributes {

            let index =
                match self.program.find_attribute(attr.name) {
                    Ok(i) => i,
                    // Don't error out if the shader doesn't use this
                    // attribute, it could be caused by shader
                    // optimization if the attribute is unused for
                    // some reason.
                    Err(Error::InvalidValue) => continue,
                    Err(e) => return Err(e),
                };

            unsafe { gl::EnableVertexAttribArray(index) };

            // This captures the buffer so that we don't have to bind it
            // when we draw later on, we'll just have to bind the vao.
            match Kind::from_type(attr.ty) {
                Kind::Integer =>
                    unsafe {
                        gl::VertexAttribIPointer(index,
                                                 attr.components,
                                                 attr.ty,
                                                 element_size,
                                                 attr.gl_offset())
                    },
                Kind::Float =>
                    unsafe {
                        gl::VertexAttribPointer(index,
                                                attr.components,
                                                attr.ty,
                                                gl::FALSE,
                                                element_size,
                                                attr.gl_offset())
                    },
                Kind::Double =>
                    unsafe {
                        gl::VertexAttribLPointer(index,
                                                 attr.components,
                                                 attr.ty,
                                                 element_size,
                                                 attr.gl_offset())
                    },
            }
        }

        get_error()
    }

    pub fn enable_attribute(&self, attr: &str) -> Result<(), Error> {
        let index = try!(self.program.find_attribute(attr));

        self.vao.bind();
        unsafe {
            gl::EnableVertexAttribArray(index);
        }

        get_error()
    }

    pub fn disable_attribute(&self, attr: &str) -> Result<(), Error> {
        let index = try!(self.program.find_attribute(attr));

        self.vao.bind();
        unsafe {
            gl::DisableVertexAttribArray(index);
        }

        get_error()
    }

    pub fn empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> DrawBuffer<T> {

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

    pub fn draw(&mut self, mode: GLenum) -> Result<(), Error> {
        self.vao.bind();
        self.program.bind();

        unsafe { gl::DrawArrays(mode, 0, self.len as GLsizei) };

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
