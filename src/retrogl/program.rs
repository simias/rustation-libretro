use std::ffi::CString;

use gl;
use gl::types::{GLint, GLuint};

use retrogl::shader::Shader;
use retrogl::error::{Error, error_or};

pub struct Program {
    id: GLuint,
}

impl Program {
    pub fn new(vertex_shader: Shader,
               fragment_shader: Shader) -> Result<Program, Error> {
        let id = unsafe { gl::CreateProgram() };

        vertex_shader.attach_to(id);
        fragment_shader.attach_to(id);

        unsafe { gl::LinkProgram(id) };

        vertex_shader.detach_from(id);
        fragment_shader.detach_from(id);

        // Check if the program linking was successful
        let mut status = gl::FALSE as GLint;
        unsafe { gl::GetProgramiv(id, gl::LINK_STATUS, &mut status) };

        if status == gl::TRUE as GLint {
            // There shouldn't be anything in glGetError but let's
            // check to make sure.
            error_or(Program { id: id })
        } else {
            error!("OpenGL program linking failed");

            match get_program_info_log(id) {
                Some(s) => error!("Program info log:\n{}", s),
                None => error!("No program info log")
            }

            Err(Error::BadProgram)
        }
    }

    pub fn find_attribute(&self, attr: &str) -> Result<GLuint, Error> {
        let cstr = CString::new(attr).unwrap();

        let index = unsafe { gl::GetAttribLocation(self.id, cstr.as_ptr()) };

        if index < 0 {
            error!("Couldn't find attribute {} in program", attr);
            return Err(Error::InvalidValue);
        }

        error_or(index as GLuint)
    }

    pub fn bind(&self) {
        unsafe { gl::UseProgram(self.id) };
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id) };
    }
}

fn get_program_info_log(id: GLuint) -> Option<String> {
    let mut log_len = 0 as GLint;

    unsafe {
        gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut log_len);
    }

    if log_len <= 0 {
        return None
    }

    let mut log = vec![0u8; log_len as usize];

    unsafe {
        gl::GetProgramInfoLog(id,
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
