//! The C ABI of FMI 3.0, transcribed by hand from the 3.0.1 headers
//! (`fmi3PlatformTypes.h`, `fmi3FunctionTypes.h`).
//!
//! The scalar typedefs are Rust's own types (`fmi3Float64` is `f64`, `fmi3Boolean` is
//! `bool`, `fmi3ValueReference` is `u32`), so they need no aliases here. The function
//! signatures live in [`export!`](crate::export!), and `tests/abi.c` checks every one of
//! them against the headers.

use core::ffi::{c_char, c_void};

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
