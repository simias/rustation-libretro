//! Logger implementation using libretro as a backend

use log;
use libretro;

use std::io::{Write, stderr};

struct RetroLogger;

impl log::Log for RetroLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            let s = ::std::fmt::format(*record.args());

            let lvl =
                match record.level() {
                    log::LogLevel::Error => libretro::log::Level::Error,
                    log::LogLevel::Warn => libretro::log::Level::Warn,
                    log::LogLevel::Info => libretro::log::Level::Info,
                    log::LogLevel::Debug => libretro::log::Level::Debug,
                    // Nothing below Debug in libretro
                    log::LogLevel::Trace => libretro::log::Level::Debug,
                };

            libretro::log::log(lvl, &s);
        }
    }
}

struct StdErrLogger;

impl log::Log for StdErrLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &log::LogRecord) {
        if self.enabled(record.metadata()) {
            let _ =
                writeln!(&mut stderr(),
                         "{} - {}",
                         record.level(),
                         record.args());
        }
    }
}

pub fn init() {
    let retrolog_ok = libretro::log::init();

    log::set_logger(|max_log_level| {
        // XXX Should we make this configurable?
        max_log_level.set(log::LogLevelFilter::max());

        if retrolog_ok {
            Box::new(RetroLogger)
        } else {
            Box::new(StdErrLogger)
        }
    }).unwrap();

    if retrolog_ok {
        info!("Logging initialized");
    } else {
        warn!("Couldn't initialize libretro logging, using stderr");
    }
}
