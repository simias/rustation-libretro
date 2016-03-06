/// This file contains the libretro definitions ported from `libretro.h`
///
/// For more details see the original well-commented C header file:
/// https://github.com/libretro/RetroArch/blob/master/libretro.h
///
/// I took the liberty to "rustify" the calling convention: I dropped
/// the `retro_` prefix (useless when you have namespaces) and
/// CamelCased the struct names.
///
/// Callback typedefs are altered in the same way and suffixed with
/// `Fn` for clarity.

use std::ptr;
use std::ffi::{CStr, CString};
use libc::{c_void, c_char, c_uint, c_float, c_double, size_t, int16_t};
use std::path::PathBuf;

pub trait Context {
    /// Get the system's audio and video parameters
    fn get_system_av_info(&self) -> SystemAvInfo;
    /// Advance the emulation state by one video frame and render it
    /// to the frontend's framebuffer
    fn render_frame(&mut self);
    /// Called when some configuration variables have been
    /// modified. The core should load the new values and change its
    /// behavior accordingly.
    fn refresh_variables(&mut self);
    /// Reset the game being played
    fn reset(&mut self);
    /// The OpenGL context has been reset, it needs to be rebuilt
    fn gl_context_reset(&mut self);
    /// The OpenGL context is about to be destroyed
    fn gl_context_destroy(&mut self);
}

/// Global context instance holding our emulator state. Libretro 1
/// doesn't support multi-instancing
static mut static_context: *mut Context = &mut dummy::Context;

unsafe fn set_context(context: Box<Context>) {
    static_context = Box::into_raw(context);
}

unsafe fn drop_context() {
    Box::from_raw(static_context);
    static_context = &mut dummy::Context;
}

fn context() -> &'static mut Context {
    unsafe {
        &mut *static_context
    }
}

#[repr(C)]
pub struct SystemInfo {
   pub library_name: *const c_char,
   pub library_version: *const c_char,
   pub valid_extensions: *const c_char,
   pub need_fullpath: bool,
   pub block_extract: bool,
}

#[repr(C)]
pub struct GameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: c_float,
}

#[repr(C)]
pub struct SystemTiming {
    pub fps: c_double,
    pub sample_rate: c_double,
}

#[repr(C)]
pub struct SystemAvInfo {
    pub geometry: GameGeometry,
    pub timing: SystemTiming,
}

pub type EnvironmentFn =
    unsafe extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;

pub type VideoRefreshFn =
    unsafe extern "C" fn(data: *const c_void,
                         width: c_uint,
                         height: c_uint,
                         pitch: size_t);
pub type AudioSampleFn =
    extern "C" fn(left: int16_t, right: int16_t);

pub type AudioSampleBatchFn =
    unsafe extern "C" fn(data: *const int16_t,
                         frames: size_t) -> size_t;

pub type InputPollFn = extern "C" fn();

pub type InputStateFn =
    extern "C" fn(port: c_uint,
                  device: c_uint,
                  index: c_uint,
                  id:c_uint) -> int16_t;

#[repr(C)]
pub struct GameInfo {
    path: *const c_char,
    data: *const c_void,
    size: size_t,
    meta: *const c_char,
}

#[repr(C)]
pub struct Variable {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct Message {
    pub msg: *const c_char,
    pub frames: c_uint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    SetMessage = 6,
    GetSystemDirectory = 9,
    SetPixelFormat = 10,
    SetHwRender = 14,
    GetVariable = 15,
    SetVariables = 16,
    GetVariableUpdate = 17,
    GetLogInterface = 27,
    SetSystemAvInfo = 32,
    SetGeometry = 37,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InputDevice {
    None = 0,
    JoyPad = 1,
    Mouse = 2,
    Keyboard = 3,
    LightGun = 4,
    Analog = 5,
    Pointer = 6,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Unknown = 0,
    Backspace = 8,
    Tab = 9,
    Clear = 12,
    Return = 13,
    Pause = 19,
    Escape = 27,
    Space = 32,
    Exclaim = 33,
    QuoteDbl = 34,
    Hash = 35,
    Dollar = 36,
    Ampersand = 38,
    Quote = 39,
    LeftParen = 40,
    RightParen = 41,
    Asterisk = 42,
    Plus = 43,
    Comma = 44,
    Minus = 45,
    Period = 46,
    Slash = 47,
    Num0 = 48,
    Num1 = 49,
    Num2 = 50,
    Num3 = 51,
    Num4 = 52,
    Num5 = 53,
    Num6 = 54,
    Num7 = 55,
    Num8 = 56,
    Num9 = 57,
    Colon = 58,
    Semicolon = 59,
    Less = 60,
    Equals = 61,
    Greater = 62,
    Question = 63,
    At = 64,
    LeftBracket = 91,
    Backslash = 92,
    RightBracket = 93,
    Caret = 94,
    Underscore = 95,
    Backquote = 96,
    A = 97,
    B = 98,
    C = 99,
    D = 100,
    E = 101,
    F = 102,
    G = 103,
    H = 104,
    I = 105,
    J = 106,
    K = 107,
    L = 108,
    M = 109,
    N = 110,
    O = 111,
    P = 112,
    Q = 113,
    R = 114,
    S = 115,
    T = 116,
    U = 117,
    V = 118,
    W = 119,
    X = 120,
    Y = 121,
    Z = 122,
    Delete = 127,
    Kp0 = 256,
    Kp1 = 257,
    Kp2 = 258,
    Kp3 = 259,
    Kp4 = 260,
    Kp5 = 261,
    Kp6 = 262,
    Kp7 = 263,
    Kp8 = 264,
    Kp9 = 265,
    KpPeriod = 266,
    KpDivide = 267,
    KpMultiply = 268,
    KpMinus = 269,
    KpPlus = 270,
    KpEnter = 271,
    KpEquals = 272,
    Up = 273,
    Down = 274,
    Right = 275,
    Left = 276,
    Insert = 277,
    Home = 278,
    End = 279,
    PageUp = 280,
    PageDown = 281,
    F1 = 282,
    F2 = 283,
    F3 = 284,
    F4 = 285,
    F5 = 286,
    F6 = 287,
    F7 = 288,
    F8 = 289,
    F9 = 290,
    F10 = 291,
    F11 = 292,
    F12 = 293,
    F13 = 294,
    F14 = 295,
    F15 = 296,
    NumLock = 300,
    CapsLock = 301,
    ScrolLock = 302,
    RShift = 303,
    LShift = 304,
    RCtrl = 305,
    LCtrl = 306,
    RAlt = 307,
    LAlt = 308,
    RMeta = 309,
    LMeta = 310,
    LSuper = 311,
    RSuper = 312,
    Mode = 313,
    Compose = 314,

    Help = 315,
    Print = 316,
    SysReq = 317,
    Break = 318,
    Menu = 319,
    Power = 320,
    Euro = 321,
    Undo = 322,
}

/// RETRO_DEVICE_ID_JOYPAD_* constants
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JoyPadButton {
    B = 0,
    Y = 1,
    Select = 2,
    Start = 3,
    Up = 4,
    Down = 5,
    Left = 6,
    Right = 7,
    A = 8,
    X = 9,
    L = 10,
    R = 11,
    L2 = 12,
    R2 = 13,
    L3 = 14,
    R3 = 15,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Xrgb1555 = 0,
    Xrgb8888 = 1,
    Rgb565 = 2,
}

pub mod hw_context {
    use std::ffi::CString;
    use libc::{uintptr_t, c_char, c_uint, c_void};
    use super::{call_environment_mut, Environment};

    pub type ResetFn = extern "C" fn();

    pub type GetCurrentFramebufferFn = extern "C" fn() -> uintptr_t;

    pub type GetProcAddressFn = extern "C" fn(sym: *const c_char) -> *const c_void;

    #[repr(C)]
    pub enum ContextType {
        None = 0,
        OpenGl = 1,
        OpenGlEs2 = 2,
        OpenGlCore = 3,
        OpenGlEs3 = 4,
        OpenGlEsVersion = 5,
    }

    #[repr(C)]
    pub struct RenderCallback {
        context_type: ContextType,
        context_reset: ResetFn,
        get_current_framebuffer: GetCurrentFramebufferFn,
        get_proc_address: GetProcAddressFn,
        depth: bool,
        stencil: bool,
        bottom_left_origin: bool,
        version_major: c_uint,
        version_minor: c_uint,
        cache_context: bool,
        context_destroy: ResetFn,
        debug_context: bool,
    }

    pub extern "C" fn reset() {
        super::context().gl_context_reset();
    }

    pub extern "C" fn context_destroy() {
        super::context().gl_context_destroy();
    }

    pub extern "C" fn dummy_get_current_framebuffer() -> uintptr_t {
        panic!("Called missing get_current_framebuffer callback");
    }

    pub extern "C" fn dummy_get_proc_address(_: *const c_char) -> *const c_void {
        panic!("Called missing get_proc_address callback");
    }

    static mut static_hw_context: RenderCallback = RenderCallback {
        context_type: ContextType::OpenGlCore,
        context_reset: reset,
        // Filled by frontend
        get_current_framebuffer: dummy_get_current_framebuffer,
        // Filled by frontend
        get_proc_address: dummy_get_proc_address,
        depth: false,
        stencil: false,
        bottom_left_origin: true,
        version_major: 3,
        version_minor: 3,
        cache_context: false,
        context_destroy: context_destroy,
        debug_context: false,
    };

    pub fn init() -> bool {
        unsafe {
            call_environment_mut(Environment::SetHwRender,
                                 &mut static_hw_context)
        }
    }

    pub fn get_proc_address(sym: &str) -> *const c_void {
        // OpenGL symbols should never contain \0 or something's very
        // wrong.
        let sym = CString::new(sym).unwrap();

        unsafe {
            (static_hw_context.get_proc_address)(sym.as_ptr() as *const c_char)
        }
    }

    pub fn get_current_framebuffer() -> uintptr_t {
        unsafe {
            (static_hw_context.get_current_framebuffer)()
        }
    }
}

pub mod log {
    use super::{call_environment_mut, Environment};
    use std::ffi::CString;
    use libc::c_char;

    #[repr(C)]
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Level {
        Debug = 0,
        Info = 1,
        Warn = 2,
        Error = 3,
    }

    /// I'm lying here for convenience: the function is really a
    /// variadic printf-like but Rust won't let me implement a
    /// variadic `dummy_log`. It doesn't matter anyway, we'll let Rust
    /// do all the formatting and simply pass a single ("%s",
    /// "formatted string").
    pub type PrintfFn = extern "C" fn(Level,
                                      *const c_char,
                                      *const c_char);

    #[repr(C)]
    pub struct Callback {
        log: PrintfFn,
    }

    extern "C" fn dummy_log(_: Level,
                            _: *const c_char,
                            _: *const c_char) {
        panic!("Called missing log callback");
    }

    static mut static_log: PrintfFn = dummy_log as PrintfFn;

    pub fn init() -> bool {
        let mut cb = Callback { log: dummy_log };

        unsafe {
            let ok = call_environment_mut(Environment::GetLogInterface,
                                          &mut cb);

            if ok {
                static_log = cb.log;
            }

            ok
        }
    }

    /// Send `msg` to the frontend's logger.
    pub fn log(lvl: Level, msg: &str) {
        // Make sure the message ends in a \n, mandated by the
        // libretro API.

        let trailing_newline =
            msg.as_bytes().last().map_or(false, |&c| c == b'\n');

        let format =
            if trailing_newline {
                // Message already contains a \n
                "%s\0"
            } else {
                "%s\n\0"
            };

        let msg = CString::new(msg);

        let cstr =
            match msg.as_ref() {
                Ok(s) => s.as_ptr(),
                // XXX we could replace \0 in the log with something
                // else instead.
                _ => b"<Invalid log message>" as *const _ as *const c_char,
            };

        unsafe {
            static_log(lvl, format.as_ptr() as *const _, cstr);
        }
    }
}

//*******************************************
// Libretro callbacks loaded by the frontend
//*******************************************

static mut video_refresh: VideoRefreshFn = dummy::video_refresh;
static mut input_poll: InputPollFn = dummy::input_poll;
static mut input_state: InputStateFn = dummy::input_state;
static mut audio_sample_batch: AudioSampleBatchFn = dummy::audio_sample_batch;
static mut environment: EnvironmentFn = dummy::environment;

//*******************************
// Higher level helper functions
//*******************************

pub fn gl_frame_done(width: u32, height: u32) {
    unsafe {
        // When using a hardware renderer we set the data pointer to
        // -1 to notify the frontend that the frame has been rendered
        // in the framebuffer.
        video_refresh(-1isize as *const _,
                      width as c_uint,
                      height as c_uint,
                      0);
    }
}

pub fn send_audio_samples(samples: &[i16]) {
    if samples.len() & 1 != 0 {
        panic!("Received an odd number of audio samples!");
    }

    let frames = (samples.len() / 2) as size_t;

    let r = unsafe {
        audio_sample_batch(samples.as_ptr(), frames)
    };

    if r != frames {
        panic!("Frontend didn't use all our samples! ({} != {})", r, frames);
    }
}

pub fn button_pressed(port: u8, b: JoyPadButton) -> bool {
    unsafe {
        input_state(port as c_uint,
                    InputDevice::JoyPad as c_uint,
                    0,
                    b as c_uint) != 0
    }
}

pub fn key_pressed(port: u8, k: Key) -> bool {
    unsafe {
        input_state(port as c_uint,
                    InputDevice::Keyboard as c_uint,
                    0,
                    k as c_uint) != 0
    }
}

pub fn get_system_directory() -> Option<PathBuf> {
    let mut path: *const c_char = ptr::null();

    let success =
        unsafe {
            call_environment_mut(Environment::GetSystemDirectory,
                                 &mut path)
        };

    if success && !path.is_null() {
        let path = unsafe { CStr::from_ptr(path) };

        build_path(path)
    } else {
        None
    }
}

pub fn set_pixel_format(format: PixelFormat) -> bool {
    let f = format as c_uint;

    unsafe {
        call_environment(Environment::SetPixelFormat, &f)
    }
}

pub fn set_geometry(geom: &GameGeometry) -> bool {
    unsafe {
        call_environment(Environment::SetGeometry, geom)
    }
}

/// Can destroy the OpenGL context!
pub unsafe fn set_system_av_info(av_info: &SystemAvInfo) -> bool {
    call_environment(Environment::SetSystemAvInfo, av_info)
}

/// Display `msg` on the screen for `nframes` frames
pub fn set_message(nframes: u32, msg: &str) {
    let msg = CString::new(msg);

    let cstr =
        match msg.as_ref() {
            Ok(s) => s.as_ptr(),
            _ => b"<Invalid log message>" as *const _ as *const c_char,
        };

    let message = Message { msg: cstr, frames: nframes as c_uint };

    unsafe {
        call_environment(Environment::SetMessage, &message);
    }
}

pub fn variables_need_update() -> bool {
    let mut needs_update = false;

    let ok =
        unsafe {
            call_environment_mut(Environment::GetVariableUpdate,
                                 &mut needs_update)
        };

    if !ok {
        panic!("Environment::GetVariableUpdate failed");
    }

    needs_update
}

/// `variables` *must* end with a `{ NULL, NULL }` marker
pub unsafe fn register_variables(variables: &[Variable]) -> bool {
    call_environment_slice(Environment::SetVariables, variables)
}

unsafe fn call_environment_mut<T>(which: Environment, var: &mut T) -> bool {
    environment(which as c_uint, var as *mut _ as *mut c_void)
}

unsafe fn call_environment<T>(which: Environment, var: &T) -> bool {
    environment(which as c_uint, var as *const _ as *mut c_void)
}

unsafe fn call_environment_slice<T>(which: Environment, var: &[T]) -> bool {
    environment(which as c_uint, var.as_ptr() as *const _ as *mut c_void)
}

/// Cast a mutable pointer into a mutable reference, return None if
/// it's NULL.
fn ptr_as_mut_ref<'a, T>(v: *mut T) -> Option<&'a mut T> {

    if v.is_null() {
        None
    } else {
        Some(unsafe { &mut *v })
    }
}

/// Cast a const pointer into a reference, return None if it's NULL.
fn ptr_as_ref<'a, T>(v: *const T) -> Option<&'a T> {

    if v.is_null() {
        None
    } else {
        Some(unsafe { &*v })
    }
}

//**********************************************
// Libretro entry points called by the frontend
//**********************************************

#[no_mangle]
pub extern "C" fn retro_api_version() -> c_uint {
    // We implement the version 1 of the API
    1
}

#[no_mangle]
pub extern "C" fn retro_set_environment(callback: EnvironmentFn) {
    unsafe {
        environment = callback
    }

    ::init_variables();
}

#[no_mangle]
pub extern "C" fn retro_set_video_refresh(callback: VideoRefreshFn) {
    unsafe {
        video_refresh = callback
    }
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample(_: AudioSampleFn) {
}

#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(callback: AudioSampleBatchFn) {
    unsafe {
        audio_sample_batch = callback
    }
}

#[no_mangle]
pub extern "C" fn retro_set_input_poll(callback: InputPollFn) {
    unsafe {
        input_poll = callback
    }
}

#[no_mangle]
pub extern "C" fn retro_set_input_state(callback: InputStateFn) {
    unsafe {
        input_state = callback
    }
}

static mut first_init: bool = true;

#[no_mangle]
pub extern "C" fn retro_init() {
    // retro_init can potentially be called several times even if the
    // library hasn't been unloaded (statics are not reset etc...)
    // which makes it rather useless in my opinion. Let's change that.

    unsafe {
        if first_init {
            ::init();
            first_init = false;
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_deinit() {
    // XXX Should I reset the callbacks to the dummy implementations
    // here?
}

#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut SystemInfo) {
    let info = ptr_as_mut_ref(info).unwrap();

    // Strings must be static and, of course, 0-terminated
    *info = ::SYSTEM_INFO;
}

#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut SystemAvInfo) {
    let info = ptr_as_mut_ref(info).unwrap();

    *info = context().get_system_av_info();
}

#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: c_uint,
                                                   _device: c_uint) {
    debug!("port device: {} {}", _port, _device);
}

#[no_mangle]
pub extern "C" fn retro_reset() {
    context().reset();
}

#[no_mangle]
pub unsafe extern "C" fn retro_run() {
    input_poll();

    let context = context();

    if variables_need_update() {
        context.refresh_variables();
    }

    context.render_frame();
}

#[no_mangle]
pub extern "C" fn retro_serialize_size() -> size_t {
    0
}

#[no_mangle]
pub extern "C" fn retro_serialize(_data: *mut c_void,
                                  _size: size_t) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_unserialize(_data: *const c_void,
                                    _size: size_t) -> bool {
    false
}

#[no_mangle]
pub extern "C" fn retro_cheat_reset() {
}

#[no_mangle]
pub fn retro_cheat_set(_index: c_uint,
                       _enabled: bool,
                       _code: *const c_char) {
}

#[no_mangle]
pub extern "C" fn retro_load_game(info: *const GameInfo) -> bool {
    let info = ptr_as_ref(info).unwrap();

    if info.path.is_null() {
        warn!("No path in GameInfo!");
        return false;
    }

    let path = unsafe { CStr::from_ptr(info.path) };

    let path =
        match build_path(path) {
            Some(p) => p,
            None => return false,
        };

    match ::load_game(path) {
        Some(c) => {
            unsafe {
                set_context(c);
            }
            true
        }
        None => {
            error!("Couldn't load game!");
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn retro_load_game_special(_type: c_uint,
                                          _info: *const GameInfo,
                                          _num_info: size_t) -> bool {
    false
}

#[no_mangle]
pub unsafe extern "C" fn retro_unload_game()  {
    drop_context();
}

#[no_mangle]
pub extern "C" fn retro_get_region() -> c_uint {
    0
}

#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: c_uint) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: c_uint) -> size_t {
    0
}

pub mod dummy {
    //! Placeholder implementation for the libretro callback in order
    //! to catch calls to those function in the function pointer has
    //! not yet been loaded.

    use libc::{c_void, c_uint, size_t, int16_t};

    pub unsafe extern "C" fn video_refresh(_: *const c_void,
                                       _: c_uint,
                                       _: c_uint,
                                       _: size_t) {
        panic!("Called missing video_refresh callback");
    }

    pub extern "C" fn input_poll() {
        panic!("Called missing input_poll callback");
    }

    pub unsafe extern "C" fn audio_sample_batch(_: *const int16_t,
                                                _: size_t) -> size_t {
        panic!("Called missing audio_sample_batch callback");
    }

    pub extern "C" fn input_state(_: c_uint,
                                  _: c_uint,
                                  _: c_uint,
                                  _: c_uint) -> int16_t {
        panic!("Called missing input_state callback");
    }

    pub unsafe extern "C" fn environment(_: c_uint, _: *mut c_void) -> bool {
        panic!("Called missing environment callback");
    }

    pub struct Context;

    impl super::Context for Context {
        fn render_frame(&mut self) {
            panic!("Called render_frame with no context!");
        }

        fn get_system_av_info(&self) -> super::SystemAvInfo {
            panic!("Called get_system_av_info with no context!");
        }

        fn refresh_variables(&mut self) {
            panic!("Called refresh_variables with no context!");
        }

        fn reset(&mut self) {
            panic!("Called reset with no context!");
        }

        fn gl_context_reset(&mut self) {
            panic!("Called context_reset with no context!");
        }

        fn gl_context_destroy(&mut self) {
            panic!("Called context_destroy with no context!");
        }
    }
}

/// Build a PathBuf from a C-string provided by the frontend. If the
/// C-string doesn't contain a valid Path encoding return
/// "None". `c_str` *must* be a valid pointer to a C-string.
#[cfg(unix)]
fn build_path(cstr: &CStr) -> Option<PathBuf> {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    // On unix I assume that the path is an arbitrary null-terminated
    // byte string
    Some(PathBuf::from(OsStr::from_bytes(cstr.to_bytes())))
}

/// Build a PathBuf from a C-string provided by the frontend. If the
/// C-string doesn't contain a valid Path encoding return
/// "None". `c_str` *must* be a valid pointer to a C-string.
#[cfg(not(unix))]
fn build_path(cstr: &CStr) -> Option<PathBuf> {
    // On Windows and other non-unices I assume that the path is utf-8
    // encoded
    match cstr.to_str() {
        Ok(s) => Some(PathBuf::from(s)),
        Err(_) => {
            error!("The frontend gave us an invalid path: {}",
                   cstr.to_string_lossy());
            None
        }
    }
}

pub unsafe fn get_variable<T, E>(var: &str,
                                 var_cstr: *const c_char,
                                 parser: fn (&str) -> Result<T, E>) -> T
{
    let mut v = Variable {
        key: var_cstr as *const _,
        value: ptr::null(),
    };

    let ok =
        call_environment_mut(Environment::GetVariable, &mut v);

    if !ok || v.value.is_null() {
        panic!("Couldn't get variable {}", var);
    }

    let value = CStr::from_ptr(v.value).to_str().unwrap();

    match parser(value) {
        Ok(v) => v,
        Err(_) => panic!("Couldn't parse variable {}", var),
    }
}

macro_rules! cstring {
    ($x:expr) => {
        concat!($x, '\0') as *const _ as *const c_char
    };
}

/// Create a structure `$st` which will be used to register and access
/// libretro variables:
///
/// ```rust
/// libretro_variables!(
///     struct MyVariables (prefix = "mycore") {
///         some_option: i32, FromStr::from_str => "Do something; 1|2|3",
///         enable_stuff: bool, parse_bool => "Enable stuff; enabled|disabled",
///     });
///
/// fn parse_bool(opt: &str) -> Result<bool, ()> {
///    match opt {
///        "true" | "enabled" | "on" => Ok(true),
///        "false" | "disabled" | "off" => Ok(false),
///        _ => Err(()),
///    }
/// }
///
/// ```
///
/// The variable names given to the frontend will be prefixed with
/// `$prefix` as mandated by libretro.
///
/// $parser must be a function that takes an &str and returns a
/// Result<T, _> where T is the option type.
///
/// The variables can then be registered with the frontend (prefrably
/// in the `init_variables` callback with:
///
/// ```rust
/// MyVariables::register();
/// ```
///
/// Individual variables can be accessed using getter functions:
///
/// ```rust
/// let value = MyVariables::some_option();
/// ```
#[macro_export]
macro_rules! libretro_variables {
    (struct $st:ident (prefix = $prefix:expr) {
        $($name:ident : $ty:ty , $parser:expr => $str:expr),+$(,)*
    }) => (
        struct $st;

        impl $st {
            fn register() {

                let variables = [
                    $($crate::libretro::Variable {
                        key: cstring!(concat!($prefix, '_', stringify!($name))),
                        value: cstring!($str),
                    }),+,
                    // End of table marker
                    $crate::libretro::Variable {
                        key: ::std::ptr::null() as *const c_char,
                        value: ::std::ptr::null() as *const c_char,
                    }
                    ];

                let ok = unsafe {
                    $crate::libretro::register_variables(&variables)
                };

                if !ok {
                    warn!("Failed to register variables");
                }
            }

            $(fn $name() -> $ty {
                let cstr = cstring!(concat!($prefix, '_', stringify!($name)));

                unsafe {
                    $crate::libretro::get_variable(stringify!($name),
                                                   cstr,
                                                   $parser)
                }
            })+
        });
}

#[macro_export]
macro_rules! libretro_message {
    ($nframes:expr, $($arg:tt)+) =>
        ($crate::libretro::set_message($nframes, &format!($($arg)+)))
}
