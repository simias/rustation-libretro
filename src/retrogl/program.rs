use gl;
use gl::types::{GLint, GLuint, GLenum};

use retrogl::error::{Error, error_or};

pub struct Shader {
    id: GLuint,
}

impl Shader {
    pub fn new(source: &str, shader_type: ShaderType) -> Result<Shader, Error> {
        let id = unsafe { gl::CreateShader(shader_type.into_gl()) };

        unsafe {
            gl::ShaderSource(id,
                             1,
                             [source.as_ptr()].as_ptr() as *const *const _,
                             [source.len() as GLint].as_ptr());
            gl::CompileShader(id);
        }

        // Check if the compilation was successful
        let mut status = gl::FALSE as GLint;
        unsafe { gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut status) };

        if status == gl::TRUE as GLint {
            // There shouldn't be anything in glGetError but let's
            // check to make sure.
            error_or(Shader {
                id: id,
            })
        } else {
            error!("{:?} shader compilation failed:\n{}", shader_type, source);

            match get_shader_info_log(id) {
                Some(s) => error!("Shader info log:\n{}", s),
                None => error!("No shader info log")
            }

            Err(Error::BadShader(shader_type))
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) };
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShaderType {
    Vertex,
    Fragment,
}

impl ShaderType {
    fn into_gl(self) -> GLenum {
        match self {
            ShaderType::Vertex => gl::VERTEX_SHADER,
            ShaderType::Fragment => gl::FRAGMENT_SHADER,
        }
    }
}

pub fn get_shader_info_log(id: GLuint) -> Option<String> {
    let mut log_len = 0 as GLint;

    unsafe {
        gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut log_len);
    }

    if log_len <= 0 {
        return None
    }

    let mut log = vec![0u8; log_len as usize];

    unsafe {
        gl::GetShaderInfoLog(id,
                             log_len,
                             &mut log_len,
                             log.as_mut_ptr() as *mut _);
    }

    if log_len <= 0 {
        return None
    }

    // The length returned by GetShaderInfoLog *excludes*
    // the ending \0 unlike the call to GetShaderiv above
    // so we can get rid of it by truncating here.
    log.truncate(log_len as usize);

    Some(String::from_utf8_lossy(&log).into_owned())
}
