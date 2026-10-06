//! The bodies of the exported C functions, generic over the model.
//!
//! Every function runs inside `catch_unwind`, so a panic becomes `fmi3Fatal` and a log
//! message instead of unwinding across the ABI. Under Scheduled Execution, every
//! function also runs inside the importer's preemption lock, so no call preempts
//! another on the same instance. Every pointer is checked before use. A
//! null instance gives `fmi3Error`. A null array is an empty slice if its count is zero,
//! and `fmi3Error` otherwise.

#![allow(unsafe_code)]

use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char};
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::abi::{
    ClockUpdateCallback, Handle, IntermediateUpdateCallback, IntervalQualifier, LogMessageCallback,
    PreemptionCallback, Status,
};
use crate::log::Logger;
use crate::{
    Carrier, CoSimulation, Fmu, Instance, Instantiation, Interface, Saved, ScheduledExecution,
    State,
};

/// What a handle points to: the instance, and the importer's preemption lock.
///
/// Scheduled Execution lets the importer preempt a call from another thread, and a
/// `&mut Instance` alive in two calls at once would be undefined behavior. So every call
/// takes the lock before it makes the `&mut`, and the instance is never preempted.
/// Holding the lock for a whole partition is allowed (FMI 3.0 §5.1.2). It costs the
/// importer the preemption of a running partition, not the order of the next ones.
struct Hosted<T: Fmu> {
    lock: PreemptionCallback,
    unlock: PreemptionCallback,
    instance: UnsafeCell<Instance<T>>,
}

impl<T: Fmu> Hosted<T> {
    fn into_handle(
        instance: Instance<T>,
        lock: PreemptionCallback,
        unlock: PreemptionCallback,
    ) -> Handle {
        let hosted = Self {
            lock,
            unlock,
            instance: UnsafeCell::new(instance),
        };
        Box::into_raw(Box::new(hosted)).cast()
    }
}

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

/// A slice from a C array. A null `data` gives an empty slice if `len` is zero, and
/// `None` otherwise.
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

/// Like [`slice`], but mutable.
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
/// `handle` is null, or was returned by an `instantiate_…::<T>` and not yet freed by
/// `free::<T>`.
pub unsafe fn with<T: Fmu>(
    handle: Handle,
    function: &'static str,
    call: impl FnOnce(&mut Instance<T>) -> Status,
) -> Status {
    // SAFETY: the caller's contract. A shared reference to `Hosted` aliases nothing
    // mutable: the instance is behind the `UnsafeCell`.
    let Some(hosted) = (unsafe { handle.cast::<Hosted<T>>().as_ref() }) else {
        return Status::Error;
    };
    let locked = hosted.lock.zip(hosted.unlock);
    if let Some((lock, _)) = locked {
        // SAFETY: the importer's callback, as it gave it.
        unsafe { lock() };
    }
    let instance = hosted.instance.get();
    // SAFETY: the lock, or a single-threaded interface, makes this the only reference.
    let status = match catch_unwind(AssertUnwindSafe(|| call(unsafe { &mut *instance }))) {
        Ok(status) => status,
        // SAFETY: as above. The closure's borrow ended when it unwound.
        Err(panic) => unsafe { &mut *instance }.poison(function, panic_message(&*panic)),
    };
    if let Some((_, unlock)) = locked {
        // SAFETY: as `lock`.
        unsafe { unlock() };
    }
    status
}

/// Fails a function this FMU does not implement, and logs why.
///
/// # Safety
///
/// Same as [`with`].
pub unsafe fn refuse<T: Fmu>(handle: Handle, function: &'static str, why: &str) -> Status {
    // SAFETY: the caller's contract.
    unsafe { with::<T>(handle, function, |i| i.refuse(function, why)) }
}

/// Logs that `interface` is not implemented, and returns a null instance.
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
/// The importer's pointers are valid as the standard requires.
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
        Instance::<T>::instantiate(token, context, Interface::CoSimulation, logger).ok()
    }));
    match made {
        Ok(Some(instance)) => Hosted::into_handle(instance, None, None),
        _ => core::ptr::null_mut(),
    }
}

/// `fmi3InstantiateScheduledExecution`. The preemption callbacks are used only if the
/// importer gives both.
///
/// # Safety
///
/// The importer's pointers are valid as the standard requires.
#[allow(clippy::too_many_arguments)]
pub unsafe fn instantiate_scheduled_execution<T: ScheduledExecution>(
    name: *const c_char,
    token: *const c_char,
    resource_path: *const c_char,
    _visible: bool,
    _logging_on: bool,
    environment: Handle,
    log: LogMessageCallback,
    _clock_update: ClockUpdateCallback,
    lock: PreemptionCallback,
    unlock: PreemptionCallback,
) -> Handle {
    // SAFETY: the caller's contract.
    let logger = unsafe { Logger::new(environment, log) };
    let made = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: the caller's contract.
        let (name, token, resource_path) =
            unsafe { (text(name), text(token), text(resource_path)) };
        let (Some(instance_name), Some(token)) = (name, token) else {
            logger.error("the instance name and token are required");
            return None;
        };
        let context = Instantiation {
            instance_name: instance_name.to_owned(),
            resource_path: resource_path.map(str::to_owned),
        };
        // `instantiate` logs its own refusal.
        Instance::<T>::instantiate(token, context, Interface::ScheduledExecution, logger).ok()
    }));
    match made {
        Ok(Some(instance)) => Hosted::into_handle(instance, lock, unlock),
        _ => core::ptr::null_mut(),
    }
}

/// `fmi3FreeInstance`.
///
/// # Safety
///
/// Same as [`with`]. The handle is not used afterwards.
pub unsafe fn free<T: Fmu>(handle: Handle) {
    if !handle.is_null() {
        // SAFETY: by the caller's contract, the handle is a `Box<Hosted<T>>`.
        let hosted = unsafe { Box::from_raw(handle.cast::<Hosted<T>>()) };
        let _ = catch_unwind(AssertUnwindSafe(|| drop(hosted)));
    }
}

/// `fmi3SetDebugLogging`.
///
/// # Safety
///
/// Same as [`with`], and `categories` holds `n` C strings.
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
/// Same as [`with`], and each array holds as many values as its count.
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
/// Same as [`with`], and each array holds as many values as its count.
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
/// Same as [`with`], and each out-pointer is null or valid for a write.
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

/// `fmi3GetFMUState`. Overwrites the importer's state if given one, else makes one.
///
/// # Safety
///
/// Same as [`with`], and `state` points to null or to a state this instance made.
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
/// Same as [`with`], and `state` is null or a state this instance made.
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

/// `fmi3FreeFMUState`. Frees the state and sets the importer's pointer to null.
///
/// # Safety
///
/// Same as [`with`], and `state` points to null or to a state this instance made.
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

/// `fmi3ActivateModelPartition`.
///
/// # Safety
///
/// Same as [`with`].
pub unsafe fn activate_model_partition<T: ScheduledExecution>(
    handle: Handle,
    clock: u32,
    time: f64,
) -> Status {
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, "fmi3ActivateModelPartition", |i| {
            i.activate(clock, time)
        })
    }
}

/// `fmi3GetIntervalDecimal`.
///
/// # Safety
///
/// Same as [`with`], and each array holds `n` values.
pub unsafe fn get_interval_decimal<T: Fmu>(
    handle: Handle,
    vrs: *const u32,
    n: usize,
    intervals: *mut f64,
    qualifiers: *mut IntervalQualifier,
) -> Status {
    let function = "fmi3GetIntervalDecimal";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            let arrays = (
                slice(vrs, n),
                slice_mut(intervals, n),
                slice_mut(qualifiers, n),
            );
            let (Some(vrs), Some(intervals), Some(qualifiers)) = arrays else {
                return i.refuse(function, "a null array with a non-zero count");
            };
            qualifiers.fill(IntervalQualifier::Unchanged);
            i.intervals(vrs, intervals)
        })
    }
}

/// `fmi3GetShiftDecimal`.
///
/// # Safety
///
/// Same as [`with`], and each array holds `n` values.
pub unsafe fn get_shift_decimal<T: Fmu>(
    handle: Handle,
    vrs: *const u32,
    n: usize,
    shifts: *mut f64,
) -> Status {
    let function = "fmi3GetShiftDecimal";
    // SAFETY: the caller's contract.
    unsafe {
        with::<T>(handle, function, |i| {
            match (slice(vrs, n), slice_mut(shifts, n)) {
                (Some(vrs), Some(shifts)) => i.shifts(vrs, shifts),
                _ => i.refuse(function, "a null array with a non-zero count"),
            }
        })
    }
}
