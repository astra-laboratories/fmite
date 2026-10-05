//! The bodies of the exported C functions, generic over the model. [`export!`](crate::export!) makes
//! the 75 `fmi3…` symbols, and each one forwards here, so the unsafe code lives in this
//! module and not in the implementor's crate.
//!
//! Every function runs inside `catch_unwind`: a panic becomes `fmi3Fatal` and a log
//! message, never an unwind across the ABI. Every pointer is checked before use: a null
//! instance is `fmi3Error`, and a null array is an empty slice when its count is zero
//! and `fmi3Error` otherwise.

#![allow(unsafe_code)]

use core::ffi::{CStr, c_char};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::abi::{Handle, IntermediateUpdateCallback, LogMessageCallback, Status};
use crate::log::Logger;
use crate::{Carrier, CoSimulation, Fmu, Instance, Instantiation, Saved, State};

/// A model that [`export!`](crate::export!) made symbols for, and what its export list declared, so
/// the description and the symbols read one list.
pub trait Exported: Fmu {
    /// The FMU state functions are exported, and `canGetAndSetFMUState` is written.
    const STATE: bool;
}

/// Makes the `fmi3…` symbols of an FMU, once, in its `cdylib`.
///
/// ```ignore
/// fmite::export!(Battery: CoSimulation + State);
/// ```
///
/// The list after the colon is checked: `State` requires `Battery: Clone`, and is a
/// compile error without it. A function of an interface or capability left out of the
/// list is still exported; it answers `fmi3Error` and logs why.
///
/// The symbols are global and must exist once per shared library, so the macro belongs
/// in the crate that builds the FMU's `cdylib`, not in a library other crates depend on.
#[macro_export]
macro_rules! export {
    ($model:ty: CoSimulation + State) => {
        $crate::export!(@symbols $model, state);
    };
    ($model:ty: CoSimulation) => {
        $crate::export!(@symbols $model, no_state);
    };
    (@capability state) => { true };
    (@capability no_state) => { false };
    (@state state, $model:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3GetFMUState(instance: H, state: *mut H) -> S {
            unsafe { X::get_fmu_state::<$model>(instance, state) }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3SetFMUState(instance: H, state: H) -> S {
            unsafe { X::set_fmu_state::<$model>(instance, state) }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3FreeFMUState(instance: H, state: *mut H) -> S {
            unsafe { X::free_fmu_state::<$model>(instance, state) }
        }
    };
    (@state no_state, $model:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3GetFMUState(instance: H, _: *mut H) -> S {
            unsafe { X::refuse::<$model>(instance, "fmi3GetFMUState", X::NO_STATE) }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3SetFMUState(instance: H, _: H) -> S {
            unsafe { X::refuse::<$model>(instance, "fmi3SetFMUState", X::NO_STATE) }
        }
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3FreeFMUState(instance: H, _: *mut H) -> S {
            unsafe { X::refuse::<$model>(instance, "fmi3FreeFMUState", X::NO_STATE) }
        }
    };
    (@get_set $model:ty, $($get:ident $set:ident $ty:ty,)*) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $get(
                instance: H, vrs: *const u32, n_vrs: usize, values: *mut $ty, n_values: usize,
            ) -> S {
                unsafe { X::get::<$model, $ty>(instance, vrs, n_vrs, values, n_values) }
            }
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $set(
                instance: H, vrs: *const u32, n_vrs: usize, values: *const $ty, n_values: usize,
            ) -> S {
                unsafe { X::set::<$model, $ty>(instance, vrs, n_vrs, values, n_values) }
            }
        )*
    };
    (@refused $model:ty, $why:ident: $($name:ident($($arg:ty),*);)*) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(instance: H, $(_: $arg),*) -> S {
                unsafe { X::refuse::<$model>(instance, stringify!($name), X::$why) }
            }
        )*
    };
    (@symbols $model:ty, $state:ident) => {
        impl $crate::export::Exported for $model {
            const STATE: bool = $crate::export!(@capability $state);
        }

        #[allow(unsafe_code, non_snake_case, clippy::missing_safety_doc)]
        const _: () = {
            use core::ffi::c_char;
            use $crate::abi::{self, Handle as H, Status as S};
            use $crate::export as X;

            #[unsafe(no_mangle)]
            pub extern "C" fn fmi3GetVersion() -> *const c_char {
                X::version()
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3SetDebugLogging(
                instance: H, _: bool, n_categories: usize, categories: *const *const c_char,
            ) -> S {
                unsafe { X::set_debug_logging::<$model>(instance, n_categories, categories) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3InstantiateModelExchange(
                _: *const c_char, _: *const c_char, _: *const c_char, _: bool, _: bool,
                environment: H, log: abi::LogMessageCallback,
            ) -> H {
                unsafe { X::refuse_instantiation(environment, log, "Model Exchange") }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3InstantiateCoSimulation(
                name: *const c_char, token: *const c_char, resource_path: *const c_char,
                visible: bool, logging_on: bool, event_mode_used: bool, early_return_allowed: bool,
                required: *const u32, n_required: usize, environment: H,
                log: abi::LogMessageCallback, intermediate: abi::IntermediateUpdateCallback,
            ) -> H {
                unsafe {
                    X::instantiate_co_simulation::<$model>(
                        name, token, resource_path, visible, logging_on, event_mode_used,
                        early_return_allowed, required, n_required, environment, log,
                        intermediate,
                    )
                }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3InstantiateScheduledExecution(
                _: *const c_char, _: *const c_char, _: *const c_char, _: bool, _: bool,
                environment: H, log: abi::LogMessageCallback, _: abi::ClockUpdateCallback,
                _: abi::PreemptionCallback, _: abi::PreemptionCallback,
            ) -> H {
                unsafe { X::refuse_instantiation(environment, log, "Scheduled Execution") }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3FreeInstance(instance: H) {
                unsafe { X::free::<$model>(instance) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3EnterInitializationMode(
                instance: H, tolerance_defined: bool, tolerance: f64, start: f64,
                stop_defined: bool, stop: f64,
            ) -> S {
                let _ = (tolerance_defined, tolerance);
                let stop = stop_defined.then_some(stop);
                unsafe { X::with::<$model>(instance, "fmi3EnterInitializationMode", |i| i.enter_initialization(start, stop)) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3ExitInitializationMode(instance: H) -> S {
                unsafe { X::with::<$model>(instance, "fmi3ExitInitializationMode", |i| i.exit_initialization()) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3Terminate(instance: H) -> S {
                unsafe { X::with::<$model>(instance, "fmi3Terminate", |i| i.terminate()) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3Reset(instance: H) -> S {
                unsafe { X::with::<$model>(instance, "fmi3Reset", |i| i.reset()) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn fmi3DoStep(
                instance: H, current: f64, size: f64, no_set_prior: bool,
                event_handling_needed: *mut bool, terminate: *mut bool, early_return: *mut bool,
                last_successful_time: *mut f64,
            ) -> S {
                unsafe {
                    X::do_step::<$model>(
                        instance, current, size, no_set_prior, event_handling_needed,
                        terminate, early_return, last_successful_time,
                    )
                }
            }

            $crate::export!(@get_set $model,
                fmi3GetFloat32 fmi3SetFloat32 f32,
                fmi3GetFloat64 fmi3SetFloat64 f64,
                fmi3GetInt8 fmi3SetInt8 i8,
                fmi3GetUInt8 fmi3SetUInt8 u8,
                fmi3GetInt16 fmi3SetInt16 i16,
                fmi3GetUInt16 fmi3SetUInt16 u16,
                fmi3GetInt32 fmi3SetInt32 i32,
                fmi3GetUInt32 fmi3SetUInt32 u32,
                fmi3GetInt64 fmi3SetInt64 i64,
                fmi3GetUInt64 fmi3SetUInt64 u64,
                fmi3GetBoolean fmi3SetBoolean bool,
            );

            $crate::export!(@state $state, $model);

            $crate::export!(@refused $model, NO_STRINGS:
                fmi3GetString(*const u32, usize, *mut *const c_char, usize);
                fmi3SetString(*const u32, usize, *const *const c_char, usize);
                fmi3GetBinary(*const u32, usize, *mut usize, *mut *const u8, usize);
                fmi3SetBinary(*const u32, usize, *const usize, *const *const u8, usize);
            );

            $crate::export!(@refused $model, NO_SERIALIZATION:
                fmi3SerializedFMUStateSize(H, *mut usize);
                fmi3SerializeFMUState(H, *mut u8, usize);
                fmi3DeserializeFMUState(*const u8, usize, *mut H);
            );

            $crate::export!(@refused $model, NO_DEPENDENCIES:
                fmi3GetNumberOfVariableDependencies(u32, *mut usize);
                fmi3GetVariableDependencies(u32, *mut usize, *mut u32, *mut usize, *mut abi::DependencyKind, usize);
            );

            $crate::export!(@refused $model, NO_DERIVATIVES:
                fmi3GetDirectionalDerivative(*const u32, usize, *const u32, usize, *const f64, usize, *mut f64, usize);
                fmi3GetAdjointDerivative(*const u32, usize, *const u32, usize, *const f64, usize, *mut f64, usize);
                fmi3GetOutputDerivatives(*const u32, usize, *const i32, *mut f64, usize);
            );

            $crate::export!(@refused $model, NO_CLOCKS:
                fmi3EnterConfigurationMode();
                fmi3ExitConfigurationMode();
                fmi3GetClock(*const u32, usize, *mut bool);
                fmi3SetClock(*const u32, usize, *const bool);
                fmi3GetIntervalDecimal(*const u32, usize, *mut f64, *mut abi::IntervalQualifier);
                fmi3GetIntervalFraction(*const u32, usize, *mut u64, *mut u64, *mut abi::IntervalQualifier);
                fmi3GetShiftDecimal(*const u32, usize, *mut f64);
                fmi3GetShiftFraction(*const u32, usize, *mut u64, *mut u64);
                fmi3SetIntervalDecimal(*const u32, usize, *const f64);
                fmi3SetIntervalFraction(*const u32, usize, *const u64, *const u64);
                fmi3SetShiftDecimal(*const u32, usize, *const f64);
                fmi3SetShiftFraction(*const u32, usize, *const u64, *const u64);
                fmi3ActivateModelPartition(u32, f64);
            );

            $crate::export!(@refused $model, NO_EVENT_MODE:
                fmi3EnterEventMode();
                fmi3EvaluateDiscreteStates();
                fmi3UpdateDiscreteStates(*mut bool, *mut bool, *mut bool, *mut bool, *mut bool, *mut f64);
                fmi3EnterStepMode();
            );

            $crate::export!(@refused $model, NO_MODEL_EXCHANGE:
                fmi3EnterContinuousTimeMode();
                fmi3CompletedIntegratorStep(bool, *mut bool, *mut bool);
                fmi3SetTime(f64);
                fmi3SetContinuousStates(*const f64, usize);
                fmi3GetContinuousStateDerivatives(*mut f64, usize);
                fmi3GetEventIndicators(*mut f64, usize);
                fmi3GetContinuousStates(*mut f64, usize);
                fmi3GetNominalsOfContinuousStates(*mut f64, usize);
                fmi3GetNumberOfEventIndicators(*mut usize);
                fmi3GetNumberOfContinuousStates(*mut usize);
            );
        };
    };
}

pub const NO_STATE: &str = "this FMU was exported without the State capability";
pub const NO_STRINGS: &str = "this FMU has no String or Binary variables";
pub const NO_SERIALIZATION: &str = "this FMU cannot serialize its state";
pub const NO_DEPENDENCIES: &str =
    "variable dependencies are in the model description, not at runtime";
pub const NO_DERIVATIVES: &str = "this FMU computes no partial derivatives";
pub const NO_CLOCKS: &str = "this FMU has no clocks";
pub const NO_EVENT_MODE: &str = "this FMU has no Event Mode";
pub const NO_MODEL_EXCHANGE: &str = "this FMU does not implement Model Exchange";

/// `fmi3GetVersion`.
#[must_use]
pub fn version() -> *const c_char {
    c"3.0".as_ptr()
}

fn panic_message(panic: &(dyn Any + Send)) -> &str {
    (panic.downcast_ref::<&str>().copied())
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("a panic with no message")
}

/// A slice from a C array: empty for null with a count of zero, `None` for null with
/// any other count.
///
/// # Safety
///
/// A non-null `data` points to `len` initialized values that outlive `'a`.
unsafe fn slice<'a, V>(data: *const V, len: usize) -> Option<&'a [V]> {
    if data.is_null() {
        return (len == 0).then_some(&[]);
    }
    // SAFETY: the caller's contract, and `data` is not null.
    Some(unsafe { core::slice::from_raw_parts(data, len) })
}

/// As [`slice`], mutable.
///
/// # Safety
///
/// A non-null `data` points to `len` values that outlive `'a`, with no other reference.
unsafe fn slice_mut<'a, V>(data: *mut V, len: usize) -> Option<&'a mut [V]> {
    if data.is_null() {
        return (len == 0).then_some(&mut []);
    }
    // SAFETY: the caller's contract, and `data` is not null.
    Some(unsafe { core::slice::from_raw_parts_mut(data, len) })
}

/// A string from C: `None` for null or for text that is not UTF-8.
///
/// # Safety
///
/// A non-null `text` is NUL-terminated and outlives `'a`.
unsafe fn text<'a>(text: *const c_char) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the caller's contract.
    unsafe { CStr::from_ptr(text) }.to_str().ok()
}

/// Runs `call` on the instance behind `handle`, catching a panic. A null handle is
/// `fmi3Error`.
///
/// # Safety
///
/// `handle` is null or a handle `instantiate_co_simulation::<T>` returned and
/// `free::<T>` has not freed.
pub unsafe fn with<T: Fmu>(
    handle: Handle,
    function: &'static str,
    call: impl FnOnce(&mut Instance<T>) -> Status,
) -> Status {
    let pointer = handle.cast::<Instance<T>>();
    if pointer.is_null() {
        return Status::Error;
    }
    // SAFETY: the caller's contract, and `pointer` is not null.
    match catch_unwind(AssertUnwindSafe(|| call(unsafe { &mut *pointer }))) {
        Ok(status) => status,
        // SAFETY: as above; the closure's borrow ended with the unwind.
        Err(panic) => unsafe { &mut *pointer }.poison(function, panic_message(&*panic)),
    }
}

/// Answers a function this FMU does not implement.
///
/// # Safety
///
/// As [`with`].
pub unsafe fn refuse<T: Fmu>(handle: Handle, function: &'static str, why: &str) -> Status {
    // SAFETY: the caller's contract.
    unsafe { with::<T>(handle, function, |i| i.refuse(function, why)) }
}

/// Logs why an interface fmite does not implement yet cannot be instantiated.
///
/// # Safety
///
/// As [`Logger::new`].
pub unsafe fn refuse_instantiation(
    environment: Handle,
    log: LogMessageCallback,
    interface: &str,
) -> Handle {
    // SAFETY: the caller's contract.
    let logger = unsafe { Logger::new(environment, log) };
    logger.error(&format!("this FMU does not implement {interface}"));
    core::ptr::null_mut()
}

/// `fmi3InstantiateCoSimulation`.
///
/// # Safety
///
/// The importer's pointers are as the standard requires.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub unsafe fn instantiate_co_simulation<T: CoSimulation>(
    name: *const c_char,
    token: *const c_char,
    resource_path: *const c_char,
    _visible: bool,
    _logging_on: bool,
    event_mode_used: bool,
    _early_return_allowed: bool,
    _required: *const u32,
    _n_required: usize,
    environment: Handle,
    log: LogMessageCallback,
    _intermediate: IntermediateUpdateCallback,
) -> Handle {
    // SAFETY: the caller's contract.
    let logger = unsafe { Logger::new(environment, log) };
    let made = catch_unwind(AssertUnwindSafe(|| {
        let refuse = |why: &str| {
            logger.error(why);
            None
        };
        // SAFETY: the caller's contract.
        let (name, token, resource_path) =
            unsafe { (text(name), text(token), text(resource_path)) };
        let (Some(instance_name), Some(token)) = (name, token) else {
            return refuse("the instance name and token are required");
        };
        if event_mode_used {
            return refuse("this FMU has no Event Mode");
        }
        let context = Instantiation {
            instance_name: instance_name.to_owned(),
            resource_path: resource_path.map(str::to_owned),
        };
        // `instantiate` logs its own refusal.
        Instance::<T>::instantiate(token, context, logger).ok()
    }));
    match made {
        Ok(Some(instance)) => Box::into_raw(Box::new(instance)).cast(),
        _ => core::ptr::null_mut(),
    }
}

/// `fmi3FreeInstance`.
///
/// # Safety
///
/// As [`with`]; the handle is not used again.
pub unsafe fn free<T: Fmu>(handle: Handle) {
    if !handle.is_null() {
        // SAFETY: the caller's contract: the handle is a `Box<Instance<T>>`.
        let instance = unsafe { Box::from_raw(handle.cast::<Instance<T>>()) };
        let _ = catch_unwind(AssertUnwindSafe(|| drop(instance)));
    }
}

/// `fmi3SetDebugLogging`.
///
/// # Safety
///
/// As [`with`], and `categories` holds `n` C strings.
pub unsafe fn set_debug_logging<T: Fmu>(
    handle: Handle,
    n: usize,
    categories: *const *const c_char,
) -> Status {
    let function = "fmi3SetDebugLogging";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            let Some(categories) = slice(categories, n) else {
                return i.refuse(function, "categories is null");
            };
            let names: Option<Vec<&str>> = categories.iter().map(|c| text(*c)).collect();
            match names {
                Some(names) => i.set_debug_logging(&names),
                None => i.refuse(function, "a category is null or not UTF-8"),
            }
        })
    }
}

/// `fmi3Get{Type}`.
///
/// # Safety
///
/// As [`with`], and the arrays hold their counts.
pub unsafe fn get<T: Fmu, V: Carrier>(
    handle: Handle,
    vrs: *const u32,
    n_vrs: usize,
    values: *mut V,
    n_values: usize,
) -> Status {
    let function = "fmi3Get";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            match (slice(vrs, n_vrs), slice_mut(values, n_values)) {
                (Some(vrs), Some(values)) => i.get(vrs, V::values_mut(values)),
                _ => i.refuse(function, "a null array with a non-zero count"),
            }
        })
    }
}

/// `fmi3Set{Type}`.
///
/// # Safety
///
/// As [`with`], and the arrays hold their counts.
pub unsafe fn set<T: Fmu, V: Carrier>(
    handle: Handle,
    vrs: *const u32,
    n_vrs: usize,
    values: *const V,
    n_values: usize,
) -> Status {
    let function = "fmi3Set";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            match (slice(vrs, n_vrs), slice(values, n_values)) {
                (Some(vrs), Some(values)) => i.set(vrs, V::values(values)),
                _ => i.refuse(function, "a null array with a non-zero count"),
            }
        })
    }
}

/// Writes `value` through `out` if it is not null.
///
/// # Safety
///
/// A non-null `out` is valid for a write.
unsafe fn write<V>(out: *mut V, value: V) {
    // SAFETY: the caller's contract.
    if let Some(out) = unsafe { out.as_mut() } {
        *out = value;
    }
}

/// `fmi3DoStep`.
///
/// # Safety
///
/// As [`with`], and the out-pointers are null or valid for a write.
#[allow(clippy::too_many_arguments)]
pub unsafe fn do_step<T: CoSimulation>(
    handle: Handle,
    current: f64,
    size: f64,
    _no_set_prior: bool,
    event_handling_needed: *mut bool,
    terminate: *mut bool,
    early_return: *mut bool,
    last_successful_time: *mut f64,
) -> Status {
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, "fmi3DoStep", |i| {
            let (status, stop, now) = i.do_step(current, size);
            write(event_handling_needed, false);
            write(terminate, stop);
            write(early_return, false);
            write(last_successful_time, now);
            status
        })
    }
}

/// `fmi3GetFMUState`: a new state, or the importer's old one overwritten.
///
/// # Safety
///
/// As [`with`]; `state` points to null or to a state this instance made.
pub unsafe fn get_fmu_state<T: State>(handle: Handle, state: *mut Handle) -> Status {
    let function = "fmi3GetFMUState";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            let Some(slot) = state.as_mut() else {
                return i.refuse(function, "the state pointer is null");
            };
            match i.save() {
                Ok(saved) => {
                    match slot.cast::<Saved<T>>().as_mut() {
                        Some(old) => *old = saved,
                        None => *slot = Box::into_raw(Box::new(saved)).cast(),
                    }
                    Status::Ok
                }
                Err(status) => status,
            }
        })
    }
}

/// `fmi3SetFMUState`.
///
/// # Safety
///
/// As [`with`]; `state` is null or a state this instance made.
pub unsafe fn set_fmu_state<T: State>(handle: Handle, state: Handle) -> Status {
    let function = "fmi3SetFMUState";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            match state.cast::<Saved<T>>().as_ref() {
                Some(saved) => i.restore(saved),
                None => i.refuse(function, "the state is null"),
            }
        })
    }
}

/// `fmi3FreeFMUState`: frees the state and nulls the importer's pointer.
///
/// # Safety
///
/// As [`with`]; `state` points to null or to a state this instance made.
pub unsafe fn free_fmu_state<T: State>(handle: Handle, state: *mut Handle) -> Status {
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, "fmi3FreeFMUState", |_| {
            if let Some(slot) = state.as_mut() {
                if !slot.is_null() {
                    drop(Box::from_raw(slot.cast::<Saved<T>>()));
                }
                *slot = core::ptr::null_mut();
            }
            Status::Ok
        })
    }
}
