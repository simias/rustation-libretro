use gl;
use gl::types::{GLint, GLenum};

pub trait GlType {
    /// Return the GL type associated to this rust type (BYTE, FLOAT,
    /// UNSIGNED_SHORT etc...)
    fn attribute_type() -> GLenum;
    /// Return the number of components
    fn components() -> GlComponents;
}

/// GL types in vertex attributes and uniforms can have between 1 and
/// 4 components. I put then in an enum to make it easier to match on
/// them.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GlComponents {
    Single = 1,
    Pair = 2,
    Triple = 3,
    Quad = 4,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Kind {
    Integer,
    Float,
    Double,
}

impl Kind {
    pub fn from_type(t: GLenum) -> Kind {
        match t {
            gl::BYTE | gl::UNSIGNED_BYTE | gl::SHORT |
            gl::UNSIGNED_SHORT | gl::INT | gl::UNSIGNED_INT
                => Kind::Integer,
            gl::FLOAT => Kind::Float,
            gl::DOUBLE => Kind::Double,
            _ => panic!("Kind of GL type {} not known", t),
        }
    }
}

impl GlComponents {
    pub fn into_gl(self) -> GLint {
        self as GLint
    }
}

impl GlType for u32 {
    fn attribute_type() -> GLenum {
        gl::UNSIGNED_INT
    }

    fn components() -> GlComponents {
        GlComponents::Single
    }
}

impl GlType for [u8; 3] {
    fn attribute_type() -> GLenum {
        gl::UNSIGNED_BYTE
    }

    fn components() -> GlComponents {
        GlComponents::Triple
    }
}

impl GlType for [i16; 2] {
    fn attribute_type() -> GLenum {
        gl::SHORT
    }

    fn components() -> GlComponents {
        GlComponents::Pair
    }
}

impl GlType for [i16; 3] {
    fn attribute_type() -> GLenum {
        gl::SHORT
    }

    fn components() -> GlComponents {
        GlComponents::Triple
    }
}

impl GlType for [u16; 2] {
    fn attribute_type() -> GLenum {
        gl::UNSIGNED_SHORT
    }

    fn components() -> GlComponents {
        GlComponents::Pair
    }
}

impl GlType for u8 {
    fn attribute_type() -> GLenum {
        gl::UNSIGNED_BYTE
    }

    fn components() -> GlComponents {
        GlComponents::Single
    }
}

impl GlType for [f32; 2] {
    fn attribute_type() -> GLenum {
        gl::FLOAT
    }

    fn components() -> GlComponents {
        GlComponents::Pair
    }
}

impl GlType for [f32; 3] {
    fn attribute_type() -> GLenum {
        gl::FLOAT
    }

    fn components() -> GlComponents {
        GlComponents::Triple
    }
}

impl GlType for [f32; 4] {
    fn attribute_type() -> GLenum {
        gl::FLOAT
    }

    fn components() -> GlComponents {
        GlComponents::Quad
    }
}
