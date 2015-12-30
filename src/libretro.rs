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
use std::ffi::CStr;
use libc::{c_void, c_char, c_uint, c_float, c_double, size_t, int16_t};
use std::path::Path;

pub trait Context {
    fn render_frame(&mut self);
    fn get_system_av_info(&self) -> SystemAvInfo;
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
    key: *const c_char,
    value: *const c_char,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    SetHwRender = 14,
    GetVariable = 15,
    SetVariables = 16,
    GetVariableUpdate = 17,
    GetLogInterface = 27,
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

pub mod hw_context {
    use libc::{uintptr_t, c_char, c_uint, c_void};
    use super::{call_environment, Environment};

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
        warn!("Context reset!");
    }

    pub extern "C" fn context_destroy() {
        unsafe {
            static_hw_context.get_current_framebuffer =
                dummy_get_current_framebuffer;
            static_hw_context.get_proc_address =
                dummy_get_proc_address;
        }

        panic!("Context destroy!");
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
            call_environment(Environment::SetHwRender, &mut static_hw_context)
        }
    }

    pub fn get_proc_address(sym: &str) -> *const c_void {
        unsafe {
            (static_hw_context.get_proc_address)(sym.as_ptr() as *const c_char)
        }
    }
}

pub mod log {
    use super::{call_environment, Environment};
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
            let ok = call_environment(Environment::GetLogInterface, &mut cb);

            if ok {
                static_log = cb.log;
            }

            ok
        }
    }

    /// Send `msg` to the frontend's logger. This function will add
    /// `\n` at the end of the message.
    pub fn log(lvl: Level, msg: &str) {
        let msg = CString::new(msg);

        let cstr =
            match msg.as_ref() {
                Ok(s) => s.as_ptr(),
                // XXX we could replace \0 in the log with something
                // else instead.
                _ => b"<Invalid log message>" as *const _ as *const c_char,
            };

        unsafe {
            // The %s makes sure the frontend won't try to interpret
            // any '%' possibly present in the log message. Libretro
            // messages should always end with a \n.
            static_log(lvl, b"%s\n" as *const _ as *const c_char, cstr);
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

pub fn frame_done(frame: [u16; 160*144]) {
    unsafe {
        let data = frame.as_ptr() as *const c_void;

        video_refresh(data, 160, 144, 160 * 2);
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

pub fn button_pressed(b: JoyPadButton) -> bool {
    unsafe {
        input_state(0,
                    InputDevice::JoyPad as c_uint,
                    0,
                    b as c_uint) != 0
    }
}

unsafe fn call_environment<T>(which: Environment, var: &mut T) -> bool {
    environment(which as c_uint, var as *mut _ as *mut c_void)
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

#[no_mangle]
pub extern "C" fn retro_init() {
    ::init()
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
    warn!("retro reset");
}

#[no_mangle]
pub unsafe extern "C" fn retro_run() {
    input_poll();

    context().render_frame();
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

    let path = unsafe {
        CStr::from_ptr(info.path)
    }.to_str().unwrap();

    if !hw_context::init() {
        return false;
    }

    match ::load_game(Path::new(path)) {
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
    }
}
