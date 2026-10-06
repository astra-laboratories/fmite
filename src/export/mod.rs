//! The exported symbols. [`export!`](crate::export!) writes the 75 `fmi3…` functions of
//! an FMU. Each one calls its body in `calls.rs`, so the unsafe code lives in this crate
//! and not in the implementor's. A function the FMU does not implement fails with one of
//! the reasons below.

mod calls;

pub use calls::*;

use crate::{Fmu, Interface};

/// A model that [`export!`](crate::export!) made symbols for. It records the export
/// list, so the model description and the symbols agree.
pub trait Exported: Fmu {
    /// The interface exported: `<CoSimulation>` or `<ScheduledExecution>`.
    const INTERFACE: Interface;
    /// Co-Simulation's `fixedInternalStepSize`.
    const FIXED_INTERNAL_STEP_SIZE: Option<f64>;
    /// Whether the FMU state functions are exported. Written as `canGetAndSetFMUState`.
    const STATE: bool;
}

/// Exports the `fmi3…` symbols of an FMU from its `cdylib`.
///
/// ```ignore
/// fmite::export!(Battery: CoSimulation + State);
/// ```
///
/// The interface is `CoSimulation` or `ScheduledExecution`, optionally `+ State`. The
/// list after the colon is checked: `State` needs `Battery: Clone`, a Scheduled
/// Execution FMU needs a clock, and a Co-Simulation FMU must have none, since FMI 3.0
/// gives a Co-Simulation FMU with clocks Event Mode, which fmite does not implement.
/// Functions of an interface or capability not in the list are still exported. They
/// return `fmi3Error` and log why.
///
/// The symbols are global and must appear once per shared library. Call the macro in the
/// crate that builds the FMU's `cdylib`, not in a library other crates depend on.
#[macro_export]
macro_rules! export {
    ($model:ty: CoSimulation + State) => {
        $crate::export!(@symbols $model, co_simulation, state);
    };
    ($model:ty: CoSimulation) => {
        $crate::export!(@symbols $model, co_simulation, no_state);
    };
    ($model:ty: ScheduledExecution + State) => {
        $crate::export!(@symbols $model, scheduled_execution, state);
    };
    ($model:ty: ScheduledExecution) => {
        $crate::export!(@symbols $model, scheduled_execution, no_state);
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
    (@interface co_simulation, $model:ty) => {
        const _: () = assert!(
            !$crate::has_clock(<$model as $crate::Variables>::VARIABLES),
            "a Co-Simulation FMU with clocks needs Event Mode, which fmite does not implement"
        );

        impl $crate::export::Exported for $model {
            const INTERFACE: $crate::Interface = $crate::Interface::CoSimulation;
            const FIXED_INTERNAL_STEP_SIZE: Option<f64> =
                <$model as $crate::CoSimulation>::FIXED_INTERNAL_STEP_SIZE;
            const STATE: bool = STATE;
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

        $crate::export!(@refused $model, NO_CLOCKS:
            fmi3GetIntervalDecimal(*const u32, usize, *mut f64, *mut abi::IntervalQualifier);
            fmi3GetShiftDecimal(*const u32, usize, *mut f64);
            fmi3ActivateModelPartition(u32, f64);
        );
    };
    (@interface scheduled_execution, $model:ty) => {
        const _: () = assert!(
            $crate::has_clock(<$model as $crate::Variables>::VARIABLES),
            "a Scheduled Execution FMU needs a clock"
        );

        impl $crate::export::Exported for $model {
            const INTERFACE: $crate::Interface = $crate::Interface::ScheduledExecution;
            const FIXED_INTERNAL_STEP_SIZE: Option<f64> = None;
            const STATE: bool = STATE;
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3InstantiateCoSimulation(
            _: *const c_char, _: *const c_char, _: *const c_char, _: bool, _: bool, _: bool,
            _: bool, _: *const u32, _: usize, environment: H, log: abi::LogMessageCallback,
            _: abi::IntermediateUpdateCallback,
        ) -> H {
            unsafe { X::refuse_instantiation(environment, log, "Co-Simulation") }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3InstantiateScheduledExecution(
            name: *const c_char, token: *const c_char, resource_path: *const c_char,
            visible: bool, logging_on: bool, environment: H, log: abi::LogMessageCallback,
            clock_update: abi::ClockUpdateCallback, lock: abi::PreemptionCallback,
            unlock: abi::PreemptionCallback,
        ) -> H {
            unsafe {
                X::instantiate_scheduled_execution::<$model>(
                    name, token, resource_path, visible, logging_on, environment, log,
                    clock_update, lock, unlock,
                )
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3ActivateModelPartition(
            instance: H, clock: u32, time: f64,
        ) -> S {
            unsafe { X::activate_model_partition::<$model>(instance, clock, time) }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3GetIntervalDecimal(
            instance: H, vrs: *const u32, n: usize, intervals: *mut f64,
            qualifiers: *mut abi::IntervalQualifier,
        ) -> S {
            unsafe { X::get_interval_decimal::<$model>(instance, vrs, n, intervals, qualifiers) }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn fmi3GetShiftDecimal(
            instance: H, vrs: *const u32, n: usize, shifts: *mut f64,
        ) -> S {
            unsafe { X::get_shift_decimal::<$model>(instance, vrs, n, shifts) }
        }

        $crate::export!(@refused $model, NO_CO_SIMULATION:
            fmi3DoStep(f64, f64, bool, *mut bool, *mut bool, *mut bool, *mut f64);
        );
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
    (@symbols $model:ty, $interface:ident, $state:ident) => {
        #[allow(unsafe_code, non_snake_case, clippy::missing_safety_doc)]
        const _: () = {
            use core::ffi::c_char;
            use $crate::abi::{self, Handle as H, Status as S};
            use $crate::export as X;

            const STATE: bool = $crate::export!(@capability $state);

            $crate::export!(@interface $interface, $model);

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

            $crate::export!(@refused $model, NO_CONFIGURATION:
                fmi3EnterConfigurationMode();
                fmi3ExitConfigurationMode();
            );

            $crate::export!(@refused $model, ONLY_PERIODIC_INPUT_CLOCKS:
                fmi3GetClock(*const u32, usize, *mut bool);
                fmi3SetClock(*const u32, usize, *const bool);
                fmi3GetIntervalFraction(*const u32, usize, *mut u64, *mut u64, *mut abi::IntervalQualifier);
                fmi3GetShiftFraction(*const u32, usize, *mut u64, *mut u64);
                fmi3SetIntervalDecimal(*const u32, usize, *const f64);
                fmi3SetIntervalFraction(*const u32, usize, *const u64, *const u64);
                fmi3SetShiftDecimal(*const u32, usize, *const f64);
                fmi3SetShiftFraction(*const u32, usize, *const u64, *const u64);
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
pub const NO_CO_SIMULATION: &str = "this FMU does not implement Co-Simulation";
pub const NO_CONFIGURATION: &str = "this FMU has no structural parameters to configure";
pub const ONLY_PERIODIC_INPUT_CLOCKS: &str = "this FMU's clocks are periodic inputs with a constant interval, \
     activated with fmi3ActivateModelPartition and not set, with no fractions";
pub const NO_EVENT_MODE: &str = "this FMU has no Event Mode";
pub const NO_MODEL_EXCHANGE: &str = "this FMU does not implement Model Exchange";
