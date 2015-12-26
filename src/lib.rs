pub mod libretro;

use libc::c_char;

extern crate libc;
extern crate rustation;

macro_rules! cstring {
    ($x:expr) => {
        concat!($x, '\0') as *const _ as *const c_char
    };
}

/// Static system information sent to the frontend on request
const SYSTEM_INFO: libretro::SystemInfo = libretro::SystemInfo {
    library_name: cstring!("Rustation"),
    library_version: rustation::VERSION_CSTR as *const _ as *const c_char,
    valid_extensions: cstring!("bin"),
    need_fullpath: false,
    block_extract: false,
};

#[test]
fn it_works() {
}
