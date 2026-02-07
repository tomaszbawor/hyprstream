use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

static LOG_LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

pub fn set_level(level: Level) {
    LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

fn enabled(level: Level) -> bool {
    level as u8 >= LOG_LEVEL.load(Ordering::Relaxed)
}

pub fn log(level: Level, msg: &str) {
    if !enabled(level) {
        return;
    }

    let prefix = match level {
        Level::Debug => "debug",
        Level::Info => "info",
        Level::Warn => "warn",
        Level::Error => "error",
    };
    eprintln!("hyprstream[{prefix}]: {msg}");
}

#[macro_export]
macro_rules! hs_debug {
    ($($arg:tt)*) => {{
        $crate::logging::log($crate::logging::Level::Debug, &format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! hs_info {
    ($($arg:tt)*) => {{
        $crate::logging::log($crate::logging::Level::Info, &format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! hs_warn {
    ($($arg:tt)*) => {{
        $crate::logging::log($crate::logging::Level::Warn, &format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! hs_error {
    ($($arg:tt)*) => {{
        $crate::logging::log($crate::logging::Level::Error, &format!($($arg)*));
    }};
}
