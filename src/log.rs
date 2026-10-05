//! Logging: the categories an FMU declares, and the importer's logger, the one thing
//! that calls back across the ABI.

#![allow(unsafe_code)]

use std::ffi::CString;

use crate::abi::{Handle, LogMessageCallback, Status};

/// The category a refused call is logged under.
pub const ERROR: &str = "logStatusError";

/// The category a panic is logged under.
pub const FATAL: &str = "logStatusFatal";

/// The categories fmite logs under, and the only ones an FMU declares. Each is a name
/// and its description, as `<LogCategories>` writes them.
pub const CATEGORIES: [(&str, &str); 2] = [
    (ERROR, "A refused call, and why"),
    (
        FATAL,
        "A panic inside the FMU; the instance takes no more calls",
    ),
];

/// The importer's logger: its callback and the environment it passes back.
pub struct Logger {
    environment: Handle,
    callback: LogMessageCallback,
}

impl Logger {
    /// A logger that drops every message, for instances made in Rust.
    #[must_use]
    pub const fn silent() -> Self {
        Self {
            environment: core::ptr::null_mut(),
            callback: None,
        }
    }

    /// # Safety
    ///
    /// `callback`, if any, must be a function the importer keeps valid for the life of
    /// the instance, and `environment` the pointer it expects back.
    #[must_use]
    pub unsafe fn new(environment: Handle, callback: LogMessageCallback) -> Self {
        Self {
            environment,
            callback,
        }
    }

    /// Logs a refused call under [`ERROR`].
    pub fn error(&self, message: &str) {
        self.log(Status::Error, ERROR, message);
    }

    /// Logs a panic under [`FATAL`].
    pub fn fatal(&self, message: &str) {
        self.log(Status::Fatal, FATAL, message);
    }

    /// Passes a message to the importer. An interior NUL, which C cannot carry, becomes
    /// a space.
    pub fn log(&self, status: Status, category: &str, message: &str) {
        let Some(callback) = self.callback else {
            return;
        };
        let c = |text: &str| CString::new(text.replace('\0', " ")).unwrap_or_default();
        let (category, message) = (c(category), c(message));
        // SAFETY: `new`'s contract makes the callback and environment valid, and both
        // strings outlive the call.
        unsafe {
            callback(
                self.environment,
                status,
                category.as_ptr(),
                message.as_ptr(),
            );
        }
    }
}
