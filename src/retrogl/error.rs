use gl;
use gl::types::{GLenum, GLint};

use retrogl::program::ShaderType;

/// OpenGL errors
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Error {
    /// Error codes returned by glGetError
    InvalidEnum,
    InvalidValue,
    InvalidOperation,
    InvalidFramebufferOperatior,
    OutOfMemory,
    /// In case we encounter an unknown OpenGL error code
    Unknown(GLenum),
    /// When shader compilation fails
    BadShader(ShaderType),
}

fn get_error() -> Result<(), Error> {
    match unsafe { gl::GetError() } {
        gl::NO_ERROR => Ok(()),
        gl::INVALID_ENUM => Err(Error::InvalidEnum),
        gl::INVALID_VALUE => Err(Error::InvalidValue),
        gl::INVALID_OPERATION => Err(Error::InvalidOperation),
        gl::INVALID_FRAMEBUFFER_OPERATION =>
            Err(Error::InvalidFramebufferOperatior),
        gl::OUT_OF_MEMORY => Err(Error::OutOfMemory),
        n => Err(Error::Unknown(n)),
    }
}

/// Return `Ok(v)` if no OpenGL error flag is active
pub fn error_or<T>(v: T) -> Result<T, Error> {

    try!(get_error());

    Ok(v)
}
