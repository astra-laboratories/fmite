//! The C ABI of FMI 3.0, transcribed by hand from the 3.0.1 headers
//! (`fmi3PlatformTypes.h`, `fmi3FunctionTypes.h`), and the one thing that calls back
//! across it: the importer's logger.
//!
//! The scalar typedefs are Rust's own types (`fmi3Float64` is `f64`, `fmi3Boolean` is
//! `bool`, `fmi3ValueReference` is `u32`), so they need no aliases here. The function
//! signatures live in [`export!`](crate::export!), and `tests/abi.c` checks every one of
//! them against the headers.

#![allow(unsafe_code)]

use core::ffi::{c_char, c_void};
use std::ffi::CString;

/// `fmi3Status`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Ok,
    Warning,
    Discard,
    Error,
    Fatal,
}

/// `fmi3DependencyKind`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyKind {
    Independent,
    Constant,
    Fixed,
    Tunable,
    Discrete,
    Dependent,
}

/// `fmi3IntervalQualifier`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntervalQualifier {
    NotYetKnown,
    Unchanged,
    Changed,
}

/// `fmi3Instance`, `fmi3InstanceEnvironment` and `fmi3FMUState`: opaque pointers.
pub type Handle = *mut c_void;

/// `fmi3LogMessageCallback`.
pub type LogMessageCallback = Option<
    unsafe extern "C" fn(
        instance_environment: Handle,
        status: Status,
        category: *const c_char,
        message: *const c_char,
    ),
>;

/// `fmi3ClockUpdateCallback`.
pub type ClockUpdateCallback = Option<unsafe extern "C" fn(instance_environment: Handle)>;

/// `fmi3IntermediateUpdateCallback`.
pub type IntermediateUpdateCallback = Option<
    unsafe extern "C" fn(
        instance_environment: Handle,
        intermediate_update_time: f64,
        intermediate_variable_set_requested: bool,
        intermediate_variable_get_allowed: bool,
        intermediate_step_finished: bool,
        can_return_early: bool,
        early_return_requested: *mut bool,
        early_return_time: *mut f64,
    ),
>;

/// `fmi3LockPreemptionCallback` and `fmi3UnlockPreemptionCallback`.
pub type PreemptionCallback = Option<unsafe extern "C" fn()>;

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

    /// Logs a refused call under `logStatusError`.
    pub fn error(&self, message: &str) {
        self.log(Status::Error, "logStatusError", message);
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
