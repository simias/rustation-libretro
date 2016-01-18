use gl;
use gl::types::{GLint, GLuint, GLenum, GLvoid};

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

pub trait Vertex {
    fn attributes() -> Vec<Attribute>;
}

pub struct Attribute {
    pub name: &'static str,
    pub offset: usize,
    /// Attribute type (BYTE, UNSIGNED_SHORT, FLOAT etc...)
    pub ty: GLenum,
    pub components: GLint,
}

impl Attribute {
    /// For some reason VertexAttribXPointer takes the offset as a
    /// pointer...
    pub fn gl_offset(&self) -> *const GLvoid {
        self.offset as *const _
    }
}

/// Retrieve the offset of `$field` in struct `$st`
macro_rules! offset_of {
    ($st: ident, $field: ident) => ({
        let null_instance: &$st = unsafe { ::std::mem::transmute(0usize) };
        let offset: usize =
            unsafe {
                ::std::mem::transmute(&null_instance.$field)
            };

        offset
    })
}

/// Build an Attribute for `$field` in struct `$st`
macro_rules! build_attribute {
    ($st: ident, $field: ident) => ({
        /// Helper function used to build an Attribute from a struct
        /// field. The first parameter is *not* a valid pointer, it's just
        /// here in order to get the proper generic type T
        fn build<T: GlType>(_invalid: *const T,
                            name: &'static str,
                            offset: usize)
                            -> $crate::retrogl::vertex::Attribute {

            $crate::retrogl::vertex::Attribute {
                name: name,
                offset: offset,
                ty: T::attribute_type(),
                components: T::components().into_gl(),
            }
        }

        let null_instance: &$st = unsafe { ::std::mem::transmute(0usize) };
        build(&null_instance.$field, stringify!($field), offset_of!($st, $field))
    })
}

/// Inspired by glium, implement the Vertex trait for fields `$field`
/// of struct `$st`
macro_rules! implement_vertex {
    ($st:ident, $($field:ident),+$(,)*) => (
        impl $crate::retrogl::vertex::Vertex for $st {
            fn attributes() -> Vec<$crate::retrogl::vertex::Attribute> {
                vec![$(build_attribute!($st, $field)),+]
            }
        }
    )
}
