use std::ffi::CString;

use gl;
use gl::types::{GLint, GLuint, GLsizei};
use std::collections::HashMap;

use retrogl::shader::Shader;
use retrogl::error::{Error, error_or, get_error};

pub struct Program {
    id: GLuint,
    /// Hash map of all the active uniforms in this program
    uniforms: UniformMap,
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
            let uniforms = try!(load_program_uniforms(id));

            // There shouldn't be anything in glGetError but let's
            // check to make sure.
            error_or(Program {
                id: id,
                uniforms: uniforms
            })
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
            error!("Couldn't find attribute \"{}\" in program", attr);
            return Err(Error::InvalidValue);
        }

        error_or(index as GLuint)
    }

    pub fn bind(&self) {
        unsafe { gl::UseProgram(self.id) };
    }

    fn uniform(&self, name: &str) -> Result<GLint, Error> {
        let e = self.uniforms.get(name)
            .map(|&u| u)
            .ok_or(Error::BadUniform);

        if e.is_err() {
            warn!("Attempted to access unknown uniform {}", name);
        }

        e
    }

    pub fn uniform1i(&self, name: &str, i: GLint) -> Result<(), Error> {
        self.bind();

        self.uniform(name)
            .map(|u| unsafe { gl::Uniform1i(u, i) })
    }

    pub fn uniform2i(&self,
                     name: &str,
                     a: GLint,
                     b: GLint) -> Result<(), Error> {
        self.bind();

        self.uniform(name)
            .map(|u| unsafe { gl::Uniform2i(u, a, b) })
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
                              log.len() as GLsizei,
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

type UniformMap = HashMap<String, GLint>;

// Return a hashmap of all uniform names contained in `program` with
// their corresponding location.
fn load_program_uniforms(program: GLuint) -> Result<UniformMap, Error> {
    let mut n_uniforms = 0;

    unsafe {
        gl::GetProgramiv(program,
                         gl::ACTIVE_UNIFORMS,
                         &mut n_uniforms as *mut GLuint as *mut _);
    }

    let mut uniforms = HashMap::with_capacity(n_uniforms as usize);

    // Figure out how long a uniform game can be
    let mut max_name_len = 0;

    unsafe {
        gl::GetProgramiv(program,
                         gl::ACTIVE_UNIFORM_MAX_LENGTH,
                         &mut max_name_len);
    }

    try!(get_error());

    for u in 0..n_uniforms {
        // Retrieve the name of this uniform
        let mut name = vec![0; max_name_len as usize];
        let mut len = 0;
        // XXX we might want to validate those at some point
        let mut size = 0;
        let mut ty = 0;

        unsafe {
            gl::GetActiveUniform(program,
                                 u,
                                 name.len() as GLsizei,
                                 &mut len,
                                 &mut size,
                                 &mut ty,
                                 name.as_mut_ptr() as *mut _);
        }

        if len <= 0 {
            warn!("Ignoring uniform name with size {}", len);
            continue;
        }

        // Retrieve the location of this uniform
        let location = unsafe {
            // GetActiveUniform puts a \0-terminated c-string in
            // `name` so we can use that directly.
            gl::GetUniformLocation(program, name.as_ptr() as *const _)
        };

        name.truncate(len as usize);
        let name = String::from_utf8(name).unwrap();

        if location < 0 {
            warn!("Uniform \"{}\" doesn't have a location", name);
            continue;
        }

        uniforms.insert(name, location);
    }

    error_or(uniforms)
}
