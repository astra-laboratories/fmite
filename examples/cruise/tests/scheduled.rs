//! Drives the cruise FMU through its exported symbols, as an importer would: instantiate
//! for Scheduled Execution, initialize, then activate the two clocks' partitions over
//! 100 ms, fastest first.

use core::ffi::{CStr, c_char, c_void};
use core::ptr::null_mut;
use std::sync::atomic::{AtomicUsize, Ordering};

use cruise::Cruise;
use fmite::Variables;
use fmite::abi::{
    ClockUpdateCallback, Handle, IntermediateUpdateCallback, IntervalQualifier, LogMessageCallback,
    PreemptionCallback, Status,
};

unsafe extern "C" {
    fn fmi3InstantiateScheduledExecution(
        name: *const c_char,
        token: *const c_char,
        resource_path: *const c_char,
        visible: bool,
        logging_on: bool,
        environment: Handle,
        log: LogMessageCallback,
        clock_update: ClockUpdateCallback,
        lock: PreemptionCallback,
        unlock: PreemptionCallback,
    ) -> Handle;
    fn fmi3InstantiateCoSimulation(
        name: *const c_char,
        token: *const c_char,
        resource_path: *const c_char,
        visible: bool,
        logging_on: bool,
        event_mode_used: bool,
        early_return_allowed: bool,
        required: *const u32,
        n_required: usize,
        environment: Handle,
        log: LogMessageCallback,
        intermediate: IntermediateUpdateCallback,
    ) -> Handle;
    fn fmi3EnterInitializationMode(
        instance: Handle,
        tolerance_defined: bool,
        tolerance: f64,
        start: f64,
        stop_defined: bool,
        stop: f64,
    ) -> Status;
    fn fmi3ExitInitializationMode(instance: Handle) -> Status;
    fn fmi3SetFloat64(
        instance: Handle,
        vrs: *const u32,
        n_vrs: usize,
        values: *const f64,
        n_values: usize,
    ) -> Status;
    fn fmi3GetFloat64(
        instance: Handle,
        vrs: *const u32,
        n_vrs: usize,
        values: *mut f64,
        n_values: usize,
    ) -> Status;
    fn fmi3ActivateModelPartition(instance: Handle, clock: u32, time: f64) -> Status;
    fn fmi3GetIntervalDecimal(
        instance: Handle,
        vrs: *const u32,
        n: usize,
        intervals: *mut f64,
        qualifiers: *mut IntervalQualifier,
    ) -> Status;
    fn fmi3DoStep(
        instance: Handle,
        current: f64,
        size: f64,
        no_set_prior: bool,
        event_handling_needed: *mut bool,
        terminate: *mut bool,
        early_return: *mut bool,
        last_successful_time: *mut f64,
    ) -> Status;
    fn fmi3Terminate(instance: Handle) -> Status;
    fn fmi3FreeInstance(instance: Handle);
}

// Value references, in field order.
const ACTUATOR: u32 = 1;
const SPEED_LOOP: u32 = 2;
const SETPOINT: u32 = 3;
const SPEED: u32 = 4;
const COMMAND: u32 = 8;
const TORQUE: u32 = 9;

static LOCKED: AtomicUsize = AtomicUsize::new(0);
static UNLOCKED: AtomicUsize = AtomicUsize::new(0);

extern "C" fn lock() {
    LOCKED.fetch_add(1, Ordering::SeqCst);
}

extern "C" fn unlock() {
    UNLOCKED.fetch_add(1, Ordering::SeqCst);
}

fn token() -> std::ffi::CString {
    std::ffi::CString::new(Cruise::INSTANTIATION_TOKEN).unwrap()
}

fn get(instance: Handle, vr: u32) -> f64 {
    let mut value = 0.0;
    let status = unsafe { fmi3GetFloat64(instance, &vr, 1, &mut value, 1) };
    assert_eq!(status, Status::Ok);
    value
}

#[test]
fn the_importer_runs_each_partition_at_its_clock() {
    let name: &CStr = c"cruise";
    let token = token();
    let instance = unsafe {
        fmi3InstantiateScheduledExecution(
            name.as_ptr(),
            token.as_ptr(),
            core::ptr::null(),
            false,
            false,
            null_mut::<c_void>(),
            None,
            None,
            Some(lock),
            Some(unlock),
        )
    };
    assert!(!instance.is_null());
    unsafe {
        assert_eq!(
            fmi3EnterInitializationMode(instance, false, 0.0, 0.0, false, 0.0),
            Status::Ok
        );
        assert_eq!(fmi3ExitInitializationMode(instance), Status::Ok);

        let mut intervals = [0.0; 2];
        let mut qualifiers = [IntervalQualifier::NotYetKnown; 2];
        let clocks = [ACTUATOR, SPEED_LOOP];
        let status = fmi3GetIntervalDecimal(
            instance,
            clocks.as_ptr(),
            2,
            intervals.as_mut_ptr(),
            qualifiers.as_mut_ptr(),
        );
        assert_eq!(status, Status::Ok);
        assert_eq!(intervals, [0.01, 0.1]);
        assert_eq!(qualifiers, [IntervalQualifier::Unchanged; 2]);

        let inputs = [SETPOINT, SPEED];
        let speeds = [20.0, 18.0];
        assert_eq!(
            fmi3SetFloat64(instance, inputs.as_ptr(), 2, speeds.as_ptr(), 2),
            Status::Ok
        );
        for tick in 0..=10_u32 {
            let time = f64::from(tick) * 0.01;
            assert_eq!(
                fmi3ActivateModelPartition(instance, ACTUATOR, time),
                Status::Ok
            );
            if tick % 10 == 0 {
                assert_eq!(
                    fmi3ActivateModelPartition(instance, SPEED_LOOP, time),
                    Status::Ok
                );
            }
            if tick == 0 {
                // The actuator ran first, on the command from before any speed loop.
                assert_eq!(get(instance, TORQUE), 0.0);
            }
        }
        // Ten actuator ticks at 10 N·m each; the second speed loop added another 0.2 m
        // to the integral.
        assert!((get(instance, TORQUE) - 100.0).abs() < 1e-9);
        assert!((get(instance, COMMAND) - 816.0).abs() < 1e-9);

        // Activations of one clock move forward.
        assert_eq!(
            fmi3ActivateModelPartition(instance, ACTUATOR, 0.05),
            Status::Error
        );
        // After an error, every call fails until Terminated.
        let mut value = 0.0;
        assert_eq!(
            fmi3GetFloat64(instance, &TORQUE, 1, &mut value, 1),
            Status::Error
        );
        assert_eq!(fmi3Terminate(instance), Status::Ok);
        fmi3FreeInstance(instance);
    }
    let locked = LOCKED.load(Ordering::SeqCst);
    assert!(locked > 0);
    assert_eq!(locked, UNLOCKED.load(Ordering::SeqCst));
}

#[test]
fn the_other_interfaces_are_refused() {
    let name: &CStr = c"cruise";
    let token = token();
    unsafe {
        let instance = fmi3InstantiateCoSimulation(
            name.as_ptr(),
            token.as_ptr(),
            core::ptr::null(),
            false,
            false,
            false,
            false,
            core::ptr::null(),
            0,
            null_mut::<c_void>(),
            None,
            None,
        );
        assert!(instance.is_null());

        let instance = fmi3InstantiateScheduledExecution(
            name.as_ptr(),
            token.as_ptr(),
            core::ptr::null(),
            false,
            false,
            null_mut::<c_void>(),
            None,
            None,
            None,
            None,
        );
        assert!(!instance.is_null());
        let status = fmi3DoStep(
            instance,
            0.0,
            0.01,
            false,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
        );
        assert_eq!(status, Status::Error);
        fmi3FreeInstance(instance);
    }
}
