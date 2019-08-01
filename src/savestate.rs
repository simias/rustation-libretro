use std::io;

pub struct Encoder<'a> {
    writer: &'a mut io::Write,
}

impl<'a> Encoder<'a> {
    pub fn new(writer: &'a mut io::Write) -> Result<Encoder<'a>, Error> {

        let mut encoder =  Encoder {
            writer: writer
        };

        // Magic
        try!(encoder.write_bytes(MAGIC));

        // It's pointless to store a version here since savestates
        // will probably break every time we make a significant change
        // to the core of the emulator.

        Ok(encoder)
    }

    fn write_bytes(&mut self, b: &[u8]) -> Result<(), Error> {
        match self.writer.write_all(b) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::IoError(e)),
        }
    }
}

impl<'a> ::rustc_serialize::Encoder for Encoder<'a> {

    type Error = Error;

    fn emit_nil(&mut self) -> Result<(), Error> {
        self.emit_str("nil")
    }

    fn emit_usize(&mut self, v: usize) -> Result<(), Error> {
        if v as u32 as usize != v {
            Err(Error::USizeOverflow(v))
        } else {
            self.emit_u32(v as u32)
        }
    }

    fn emit_u64(&mut self, v: u64) -> Result<(), Error> {
        let b = [
            v as u8,
            (v >> 8) as u8,
            (v >> 16) as u8,
            (v >> 24) as u8,
            (v >> 32) as u8,
            (v >> 40) as u8,
            (v >> 48) as u8,
            (v >> 56) as u8,
        ];

        self.write_bytes(&b)
    }

    fn emit_u32(&mut self, v: u32) -> Result<(), Error> {
        let b = [
            v as u8,
            (v >> 8) as u8,
            (v >> 16) as u8,
            (v >> 24) as u8,
        ];

        self.write_bytes(&b)
    }

    fn emit_u16(&mut self, v: u16) -> Result<(), Error> {
        let b = [
            v as u8,
            (v >> 8) as u8,
        ];

        self.write_bytes(&b)
    }

    fn emit_u8(&mut self, v: u8) -> Result<(), Error> {
        self.write_bytes(&[v])
    }

    fn emit_isize(&mut self, v: isize) -> Result<(), Error> {
        if v as i32 as isize != v {
            Err(Error::ISizeOverflow(v))
        } else {
            self.emit_i32(v as i32)
        }
    }

    fn emit_i64(&mut self, v: i64) -> Result<(), Error> {
        self.emit_u64(v as u64)
    }

    fn emit_i32(&mut self, v: i32) -> Result<(), Error> {
        self.emit_u32(v as u32)
    }

    fn emit_i16(&mut self, v: i16) -> Result<(), Error> {
        self.emit_u16(v as u16)
    }

    fn emit_i8(&mut self, v: i8) -> Result<(), Error> {
        self.emit_u8(v as u8)
    }

    fn emit_bool(&mut self, v: bool) -> Result<(), Error> {
        self.emit_u8(v as u8)
    }

    fn emit_f64(&mut self, _: f64) -> Result<(), Error> {
        panic!("f64 serialization")
    }

    fn emit_f32(&mut self, _: f32) -> Result<(), Error> {
        panic!("f32 serialization")
    }

    fn emit_char(&mut self, v: char) -> Result<(), Error> {
        self.emit_u32(v as u32)
    }

    fn emit_str(&mut self, v: &str) -> Result<(), Error> {
        // Convert into bytes
        let s = v.as_bytes();

        let len = s.len();

        if len > STRING_MAX_LEN {
            return Err(Error::StringTooLong(len));
        }

        try!(self.emit_usize(len));
        try!(self.write_bytes(s));

        Ok(())
    }

    fn emit_enum<F>(&mut self, name: &str, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        try!(self.emit_str(name));

        f(self)
    }

    fn emit_enum_variant<F>(&mut self,
                            v_name: &str,
                            _v_id: usize,
                            _len: usize,
                            f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        // We store the name instead of the ID in order not to end up
        // with messed up state if an enum gets reordered or something
        try!(self.emit_str(v_name));

        f(self)
    }

    fn emit_enum_variant_arg<F>(&mut self,
                                a_idx: usize,
                                f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        try!(self.emit_usize(a_idx));

        f(self)
    }

    fn emit_enum_struct_variant<F>(&mut self,
                                   _v_name: &str,
                                   _v_id: usize,
                                   _len: usize,
                                   _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }

    fn emit_enum_struct_variant_field<F>(&mut self,
                                         _f_name: &str,
                                         _f_idx: usize,
                                         _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }

    fn emit_struct<F>(&mut self,
                      name: &str,
                      _: usize,
                      f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        try!(self.emit_str(name));

        f(self)
    }

    fn emit_struct_field<F>(&mut self,
                            f_name: &str,
                            _: usize,
                            f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        try!(self.emit_str(f_name));

        f(self)
    }

    fn emit_tuple<F>(&mut self, len: usize, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        self.emit_seq(len, f)
    }

    fn emit_tuple_arg<F>(&mut self, idx: usize, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        self.emit_seq_elt(idx, f)
    }

    fn emit_tuple_struct<F>(&mut self, _name: &str, _len: usize, _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }

    fn emit_tuple_struct_arg<F>(&mut self, _f_idx: usize, _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }

    fn emit_option<F>(&mut self, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        f(self)
    }

    fn emit_option_none(&mut self) -> Result<(), Error> {
        self.emit_bool(false)
    }

    fn emit_option_some<F>(&mut self, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        try!(self.emit_bool(true));

        f(self)
    }

    fn emit_seq<F>(&mut self, len: usize, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        try!(self.emit_usize(len));

        f(self)
    }

    fn emit_seq_elt<F>(&mut self, _: usize, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        f(self)
    }

    fn emit_map<F>(&mut self, len: usize, f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {

        try!(self.emit_usize(len));

        f(self)
    }

    fn emit_map_elt_key<F>(&mut self, _idx: usize, _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }

    fn emit_map_elt_val<F>(&mut self, _idx: usize, _f: F) -> Result<(), Error>
        where F: FnOnce(&mut Self) -> Result<(), Error> {
        panic!()
    }
}

/// Rustation savestate format deserializer
pub struct Decoder<'a> {
    reader: &'a mut io::Read,
}

impl<'a> Decoder<'a> {
    pub fn new(reader: &'a mut io::Read) -> Result<Decoder<'a>, Error> {

        let mut decoder = Decoder {
            reader: reader,
        };

        // Check that the magic is valid
        let mut magic = [0; 4];

        try!(decoder.read_bytes(&mut magic));

        if magic != MAGIC {
            Err(Error::BadMagic)
        } else {
            Ok(decoder)
        }
    }

    fn read_bytes(&mut self, b: &mut [u8]) -> Result<(), Error> {
        match self.reader.read_exact(b) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::IoError(e)),
        }
    }

    /// Validate that an expected symbol matches the file value
    fn validate_symbol(&mut self, expected: &str) -> Result<(), Error> {
        use rustc_serialize::Decoder;

        let s = try!(self.read_str());

        if s != expected {
            Err(Error::BadSymbol(expected.into(), s))
        } else {
            Ok(())
        }
    }
}

impl<'a> ::rustc_serialize::Decoder for Decoder<'a> {
    type Error = Error;

    fn read_nil(&mut self) -> Result<(), Error> {
        self.validate_symbol("nil")
    }

    fn read_usize(&mut self) -> Result<usize, Error> {
        // usize are stored like u32s
        self.read_u32().map(|v| v as usize)
    }

    fn read_u64(&mut self) -> Result<u64, Error> {
        let mut b = [0; 8];

        try!(self.read_bytes(&mut b));

        let mut v = 0;

        for &b in b.iter().rev() {
            v <<= 8;
            v |= b as u64;
        }

        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32, Error> {
        let mut b = [0; 4];

        try!(self.read_bytes(&mut b));

        let mut v = 0;

        for &b in b.iter().rev() {
            v <<= 8;
            v |= b as u32;
        }

        Ok(v)
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let mut b = [0; 2];

        try!(self.read_bytes(&mut b));

        let mut v = 0;

        for &b in b.iter().rev() {
            v <<= 8;
            v |= b as u16;
        }

        Ok(v)
    }

    fn read_u8(&mut self) -> Result<u8, Error> {
        let mut b = [0];

        try!(self.read_bytes(&mut b));

        Ok(b[0])
    }

    fn read_isize(&mut self) -> Result<isize, Error> {
        self.read_usize().map(|v| v as isize)
    }

    fn read_i64(&mut self) -> Result<i64, Error> {
        self.read_u64().map(|v| v as i64)
    }

    fn read_i32(&mut self) -> Result<i32, Error> {
        self.read_u32().map(|v| v as i32)
    }

    fn read_i16(&mut self) -> Result<i16, Error> {
        self.read_u16().map(|v| v as i16)
    }

    fn read_i8(&mut self) -> Result<i8, Error> {
        self.read_u8().map(|v| v as i8)
    }

    fn read_bool(&mut self) -> Result<bool, Error> {
        match try!(self.read_u8()) {
            0 => Ok(false),
            1 => Ok(true),
            n => Err(Error::BadBool(n)),
        }
    }

    fn read_f64(&mut self) -> Result<f64, Error> {
        panic!()
    }

    fn read_f32(&mut self) -> Result<f32, Error> {
        panic!()
    }

    fn read_char(&mut self) -> Result<char, Error> {
        let c = try!(self.read_u32());

        ::std::char::from_u32(c).ok_or(Error::BadChar(c))
    }

    fn read_str(&mut self) -> Result<String, Error> {
        // First read the string length
        let len = try!(self.read_usize());

        if len > STRING_MAX_LEN {
            return Err(Error::StringTooLong(len));
        }

        let mut buf = vec![0; len];

        // Now we can read the string itself
        try!(self.read_bytes(&mut buf));

        // Finally we can convert the bytes to a String
        String::from_utf8(buf).map_err(|e| Error::BadString(e))
    }

    fn read_enum<T, F>(&mut self, name: &str, f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        try!(self.validate_symbol(name));

        f(self)
    }

    fn read_enum_variant<T, F>(&mut self,
                               names: &[&str],
                               mut f: F) -> Result<T, Error>
        where F: FnMut(&mut Self, usize) -> Result<T, Error> {

        let name = try!(self.read_str());

        match names.iter().position(|n| *n == name) {
            Some(id) => f(self, id),
            None => Err(Error::BadEnumVariant(name)),
        }
    }

    fn read_enum_variant_arg<T, F>(&mut self,
                                   a_idx: usize,
                                   f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        let id = try!(self.read_usize());

        if id == a_idx {
            f(self)
        } else {
            Err(Error::BadEnumVariantId(a_idx, id))
        }
    }

    fn read_enum_struct_variant<T, F>(&mut self,
                                      _names: &[&str],
                                      _f: F) -> Result<T, Error>
        where F: FnMut(&mut Self, usize) -> Result<T, Error> {
        panic!()
    }

    fn read_enum_struct_variant_field<T, F>(&mut self,
                                            _f_name: &str,
                                            _f_idx: usize,
                                            _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {
        panic!()
    }

    fn read_struct<T, F>(&mut self,
                         s_name: &str,
                         _: usize,
                         f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        try!(self.validate_symbol(s_name));

        f(self)
    }

    fn read_struct_field<T, F>(&mut self,
                               f_name: &str,
                               _: usize,
                               f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        try!(self.validate_symbol(f_name));

        f(self)
    }

    fn read_tuple<T, F>(&mut self, len: usize, f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        self.read_seq(|d, l| {
            if l == len {
                f(d)
            } else {
                Err(Error::BadTupleLength(len, l))
            }
        })
    }

    fn read_tuple_arg<T, F>(&mut self, a_idx: usize, f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        self.read_seq_elt(a_idx, f)
    }

    fn read_tuple_struct<T, F>(&mut self, _s_name: &str, _len: usize, _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {
        panic!()
    }

    fn read_tuple_struct_arg<T, F>(&mut self, _a_idx: usize, _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {
        panic!()
    }

    fn read_option<T, F>(&mut self, mut f: F) -> Result<T, Error>
        where F: FnMut(&mut Self, bool) -> Result<T, Error> {

        let is_some = try!(self.read_bool());

        f(self, is_some)
    }

    fn read_seq<T, F>(&mut self, f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self, usize) -> Result<T, Error> {

        let len = try!(self.read_usize());

        f(self, len)
    }

    fn read_seq_elt<T, F>(&mut self, _: usize, f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {

        // XXX I assume reads are done sequentially starting from 0,
        // so I ignore idx

        f(self)
    }

    fn read_map<T, F>(&mut self, _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self, usize) -> Result<T, Error> {
        panic!()
    }

    fn read_map_elt_key<T, F>(&mut self, _idx: usize, _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {
        panic!()
    }

    fn read_map_elt_val<T, F>(&mut self, _idx: usize, _f: F) -> Result<T, Error>
        where F: FnOnce(&mut Self) -> Result<T, Error> {
        panic!()
    }

    fn error(&mut self, err: &str) -> Error {
        Error::ApplicationError(err.into())
    }

}

#[derive(Debug)]
/// Error type used by the encoder and decoder
pub enum Error {
    /// Savestate format has invalid magic
    BadMagic,
    /// Error while reading or writing the savestate
    IoError(io::Error),
    /// Encountered an unexpected symbol: `(expected, got)`
    BadSymbol(String, String),
    /// String conversion failed
    BadString(::std::string::FromUtf8Error),
    /// usize is too big to be serialized
    USizeOverflow(usize),
    /// isize is too big to be serialized
    ISizeOverflow(isize),
    /// Error reported by application
    ApplicationError(String),
    /// Attempted to encode or decode an unreasonably large string
    StringTooLong(usize),
    /// Encountered unknown enum variant while decoding
    BadEnumVariant(String),
    /// Encountered an unexpected enum variant argument id while
    /// decoding: `(expected, got)`
    BadEnumVariantId(usize, usize),
    /// Encountered an invalid char while decoding
    BadChar(u32),
    /// Encountered an invalid tuple length while decoding:
    /// `(expected, got)`
    BadTupleLength(usize, usize),
    /// Encountered an invalid bool while decoding
    BadBool(u8),
}

/// "Magic" string stored in the header to indentify the file format
pub const MAGIC: &'static [u8] = b"RSXB";
/// Maximum string length accepted by the format. This is especially
/// useful while decoding a bogus savestate, we don't want to allocate
/// a huge string only to discover that there's a missmatch later.
pub const STRING_MAX_LEN: usize = 1024 * 1024;


#[test]
fn test_serialize_deserialize() {
    use rustc_serialize::{Encodable, Decodable};

    #[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
    enum Enum {
        A,
        B,
        C,
    }

    #[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
    enum EnumArgs {
        X(u32),
        Y(String, u8),
        Z(Vec<char>, Enum, i16),
    }

    #[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
    struct Struct {
        field: u32,
        field2: i32,
    }

    #[derive(RustcDecodable, RustcEncodable, Debug, PartialEq, Eq)]
    struct TestStruct  {
        data_int: u8,
        data_str: String,
        data_vector: Vec<u16>,
        data_struct: Struct,
        data_enum: Enum,
        data_enum_args: EnumArgs,
        data_tuple: (u64, char, bool, ()),
        data_option: Option<i64>,
    }

    let object = TestStruct {
        data_int: 1,
        data_str: "homura".to_string(),
        data_vector: vec![2, 3, 4, 5],
        data_struct: Struct {
            field: 0x42,
            field2: -1,
        },
        data_enum: Enum::B,
        data_enum_args: EnumArgs::Z(vec!['@', 'a', 'é', 'π'], Enum::C, -3),
        data_tuple: (1234, '!', true, ()),
        data_option: Some(-4335),
    };

    let mut serialized = Vec::new();

    {
        let mut encoder = Encoder::new(&mut serialized).unwrap();

        object.encode(&mut encoder).unwrap();
    }

    let mut reader: &[u8] = &serialized;

    let mut decoder = Decoder::new(&mut reader).unwrap();

    let decoded: TestStruct = Decodable::decode(&mut decoder).unwrap();

    assert_eq!(decoded, object);
}
