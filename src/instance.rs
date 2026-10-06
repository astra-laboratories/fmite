//! `Instance<T>`: one instantiated FMU. It checks each importer call against its
//! interface's state machine and `T::VARIABLES`. So `T` only sees calls the standard
//! allows, with value references it knows and values of the right type.

use crate::abi::Status;
use crate::co_simulation::Timeline;
use crate::log::{self, Logger};
use crate::values::surplus;
use crate::{
    Activation, CoSimulation, Error, Fmu, Instantiation, Interface, Kind, ScheduledExecution,
    State, Step, StepResult, ValueReference, Values, ValuesMut, Variable, check,
};

/// Where the instance is in its interface's state machine. Each variant holds only
/// what its mode needs: there is no time before initialization and no timeline before
/// Step Mode.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Instantiated,
    Initialization {
        start: f64,
    },
    Step(Timeline),
    /// Scheduled Execution's Step Mode. `now` is the latest activation's time.
    ClockActivation {
        now: f64,
    },
    /// A call failed under Scheduled Execution. Until `fmi3Terminate` or `fmi3Reset`,
    /// every other call fails too (FMI 3.0 §5.2).
    Failed {
        now: f64,
    },
    Terminated {
        now: f64,
    },
    /// A call panicked. The model may be half-updated, so only freeing is allowed.
    Fatal,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Instantiated => "Instantiated",
            Self::Initialization { .. } => "Initialization Mode",
            Self::Step(_) => "Step Mode",
            Self::ClockActivation { .. } => "Clock Activation Mode",
            Self::Failed { .. } => "the state after a failed call",
            Self::Terminated { .. } => "Terminated",
            Self::Fatal => "an error state",
        }
    }
}

/// A saved FMU state: the model, the instance's mode, and each clock's latest
/// activation.
pub struct Saved<T> {
    model: T,
    mode: Mode,
    activations: Vec<(u32, f64)>,
}

/// One instantiated FMU.
pub struct Instance<T: Fmu> {
    model: T,
    context: Instantiation,
    interface: Interface,
    logger: Logger,
    mode: Mode,
    /// Each clock's latest activation time, by value reference.
    activations: Vec<(u32, f64)>,
    /// Whether an `fmi3Set` came after the latest activation. An `fmi3Get` must not
    /// follow it until the next activation (FMI 3.0 §5.2.1).
    set_since_activation: bool,
}

impl<T: Fmu> Instance<T> {
    /// Checks the importer's token against `T`'s, and instantiates `T` for `interface`.
    ///
    /// # Errors
    ///
    /// If the token does not match or `T::instantiate` fails. The error is also logged.
    pub fn instantiate(
        token: &str,
        context: Instantiation,
        interface: Interface,
        logger: Logger,
    ) -> Result<Self, Error> {
        const { check(T::VARIABLES) };
        let refuse = |error: Error| {
            logger.error(error.message());
            error
        };
        if token != T::INSTANTIATION_TOKEN {
            return Err(refuse(Error::new(format!(
                "instantiation token {token} is not this FMU's, {}",
                T::INSTANTIATION_TOKEN
            ))));
        }
        let model = T::instantiate(&context).map_err(refuse)?;
        Ok(Self {
            model,
            context,
            interface,
            logger,
            mode: Mode::Instantiated,
            activations: Vec::new(),
            set_since_activation: false,
        })
    }

    /// The model, for tests that drive the instance from Rust and check the result.
    pub fn model(&self) -> &T {
        &self.model
    }

    /// Returns `fmi3OK`, or logs the error and returns `fmi3Error`. Under Scheduled
    /// Execution an error also fails the instance until `fmi3Terminate` or `fmi3Reset`.
    fn answer(&mut self, result: Result<(), Error>) -> Status {
        match result {
            Ok(()) => Status::Ok,
            Err(error) => {
                self.logger.error(error.message());
                if self.interface == Interface::ScheduledExecution
                    && !matches!(self.mode, Mode::Terminated { .. } | Mode::Fatal)
                {
                    self.mode = Mode::Failed { now: self.now() };
                }
                Status::Error
            }
        }
    }

    fn not_in(&self, function: &str) -> Error {
        Error::new(format!("{function} is not allowed in {}", self.mode.name()))
    }

    /// Fails a function this FMU does not implement, and logs why.
    pub fn refuse(&mut self, function: &str, why: &str) -> Status {
        self.answer(Err(Error::new(format!("{function}: {why}"))))
    }

    /// Records a panic. It is logged as fatal, and the instance accepts no more calls.
    pub fn poison(&mut self, function: &str, panic: &str) -> Status {
        self.mode = Mode::Fatal;
        self.logger.fatal(&format!("{function} panicked: {panic}"));
        Status::Fatal
    }

    /// `fmi3SetDebugLogging`. fmite has no debug messages, so this only checks that each
    /// category is one the FMU declares.
    pub fn set_debug_logging(&mut self, categories: &[&str]) -> Status {
        let unknown = (categories.iter())
            .find(|category| !log::CATEGORIES.iter().any(|(name, _)| name == *category));
        match unknown {
            Some(category) => self.answer(Err(Error::new(format!(
                "this FMU declares no log category {category}"
            )))),
            None => Status::Ok,
        }
    }

    /// `fmi3EnterInitializationMode`.
    pub fn enter_initialization(&mut self, start: f64, stop: Option<f64>) -> Status {
        let result = match self.mode {
            Mode::Instantiated => self.model.enter_initialization(start, stop).map(|()| {
                self.mode = Mode::Initialization { start };
            }),
            _ => Err(self.not_in("fmi3EnterInitializationMode")),
        };
        self.answer(result)
    }

    /// `fmi3ExitInitializationMode`.
    pub fn exit_initialization(&mut self) -> Status {
        let result = match self.mode {
            Mode::Initialization { start } => self.model.exit_initialization().map(|()| {
                self.mode = match self.interface {
                    Interface::CoSimulation => Mode::Step(Timeline::starting(start)),
                    Interface::ScheduledExecution => Mode::ClockActivation { now: start },
                };
            }),
            _ => Err(self.not_in("fmi3ExitInitializationMode")),
        };
        self.answer(result)
    }

    /// `fmi3Terminate`.
    pub fn terminate(&mut self) -> Status {
        let result = match self.mode {
            Mode::Step(_) | Mode::ClockActivation { .. } | Mode::Failed { .. } => {
                let now = self.now();
                (self.model.terminate()).map(|()| self.mode = Mode::Terminated { now })
            }
            _ => Err(self.not_in("fmi3Terminate")),
        };
        self.answer(result)
    }

    /// `fmi3Reset`. Instantiates the model again from the same context.
    pub fn reset(&mut self) -> Status {
        let result = match self.mode {
            Mode::Fatal => Err(self.not_in("fmi3Reset")),
            _ => T::instantiate(&self.context).map(|model| {
                self.model = model;
                self.mode = Mode::Instantiated;
                self.activations.clear();
                self.set_since_activation = false;
            }),
        };
        self.answer(result)
    }

    fn variable(vr: u32) -> Result<&'static Variable, Error> {
        (T::VARIABLES.iter())
            .find(|variable| variable.value_reference == vr)
            .ok_or_else(|| ValueReference(vr).unknown())
    }

    /// The current time: the value of `time`.
    fn now(&self) -> f64 {
        match self.mode {
            Mode::Initialization { start } => start,
            Mode::Step(timeline) => timeline.now,
            Mode::ClockActivation { now } | Mode::Failed { now } | Mode::Terminated { now } => now,
            Mode::Instantiated | Mode::Fatal => 0.0,
        }
    }

    /// `fmi3Get{Type}`. Fills `out` with the values of each reference, in order.
    pub fn get(&mut self, vrs: &[u32], out: ValuesMut<'_>) -> Status {
        let result = self.try_get(vrs, out);
        self.answer(result)
    }

    fn try_get(&mut self, vrs: &[u32], mut out: ValuesMut<'_>) -> Result<(), Error> {
        match self.mode {
            Mode::ClockActivation { .. } if self.set_since_activation => {
                return Err(Error::new(
                    "fmi3Get after fmi3Set needs an fmi3ActivateModelPartition between them",
                ));
            }
            Mode::Initialization { .. }
            | Mode::Step(_)
            | Mode::ClockActivation { .. }
            | Mode::Terminated { .. } => {}
            Mode::Instantiated | Mode::Failed { .. } | Mode::Fatal => {
                return Err(self.not_in("fmi3Get"));
            }
        }
        for &vr in vrs {
            if vr == 0 {
                let Some(ValuesMut::Float64([slot, ..])) = out.split_front(1) else {
                    return Err(Error::new("time is a Float64 variable"));
                };
                *slot = self.now();
                continue;
            }
            let variable = Self::variable(vr)?;
            let chunk = variable.values_in(out.type_name(), out.len())?;
            let buffer = out.split_front(chunk).expect("chunk checked the length");
            self.model.get(ValueReference(vr), buffer)?;
        }
        surplus(out.len())
    }

    /// `fmi3Set{Type}`. `values` holds the values for each reference, in order.
    pub fn set(&mut self, vrs: &[u32], values: Values<'_>) -> Status {
        let result = self.try_set(vrs, values);
        self.answer(result)
    }

    fn try_set(&mut self, vrs: &[u32], mut values: Values<'_>) -> Result<(), Error> {
        for &vr in vrs {
            let variable = Self::variable(vr)?;
            let allowed = match self.mode {
                Mode::Instantiated | Mode::Initialization { .. } => {
                    variable.settable_in_initialization
                }
                Mode::Step(_) | Mode::ClockActivation { .. } => variable.settable_in_step,
                Mode::Failed { .. } | Mode::Terminated { .. } | Mode::Fatal => false,
            };
            if !allowed {
                return Err(Error::new(format!(
                    "{} cannot be set in {}",
                    variable.name,
                    self.mode.name()
                )));
            }
            let chunk = variable.values_in(values.type_name(), values.len())?;
            let slice = values.split_front(chunk).expect("chunk checked the length");
            self.model.set(ValueReference(vr), slice)?;
            self.set_since_activation = matches!(self.mode, Mode::ClockActivation { .. });
        }
        surplus(values.len())
    }

    /// `fmi3GetFMUState`.
    ///
    /// # Errors
    ///
    /// In a mode where the state cannot be saved.
    pub fn save(&mut self) -> Result<Saved<T>, Status>
    where
        T: State,
    {
        match self.mode {
            Mode::Instantiated | Mode::Failed { .. } | Mode::Fatal => {
                Err(self.answer(Err(self.not_in("fmi3GetFMUState"))))
            }
            mode => Ok(Saved {
                model: self.model.clone(),
                mode,
                activations: self.activations.clone(),
            }),
        }
    }

    /// `fmi3SetFMUState`.
    pub fn restore(&mut self, saved: &Saved<T>) -> Status
    where
        T: State,
    {
        if matches!(self.mode, Mode::Failed { .. } | Mode::Fatal) {
            return self.answer(Err(self.not_in("fmi3SetFMUState")));
        }
        self.model.clone_from(&saved.model);
        self.mode = saved.mode;
        self.activations.clone_from(&saved.activations);
        self.set_since_activation = false;
        Status::Ok
    }
}

impl<T: CoSimulation> Instance<T> {
    /// `fmi3DoStep`. Returns the status, whether the model asks to stop, and the time.
    pub fn do_step(&mut self, current: f64, size: f64) -> (Status, bool, f64) {
        let Mode::Step(mut timeline) = self.mode else {
            let status = self.answer(Err(self.not_in("fmi3DoStep")));
            return (status, false, self.now());
        };
        let step = Step { current, size };
        let result = timeline.check_start(current).and_then(|()| {
            let mut next = timeline;
            next.advance(step, T::FIXED_INTERNAL_STEP_SIZE)?;
            let result = self.model.do_step(step)?;
            timeline = next;
            Ok(result)
        });
        self.mode = Mode::Step(timeline);
        match result {
            Ok(result) => (Status::Ok, result == StepResult::Terminate, timeline.now),
            Err(error) => (self.answer(Err(error)), false, timeline.now),
        }
    }
}

impl<T: Fmu> Instance<T> {
    /// The clock behind a value reference.
    fn clock(vr: u32) -> Result<&'static Variable, Error> {
        let variable = Self::variable(vr)?;
        match variable.kind {
            Kind::Clock => Ok(variable),
            _ => Err(Error::new(format!("{} is not a clock", variable.name))),
        }
    }

    /// `fmi3GetIntervalDecimal`. Every clock is periodic with a constant interval, so
    /// the qualifier is always `fmi3IntervalUnchanged`, as in the standard's own
    /// example (FMI 3.0 §5.3.2).
    pub fn intervals(&mut self, vrs: &[u32], out: &mut [f64]) -> Status {
        let result = self.clock_values(vrs, out, |clock| clock.interval.as_secs_f64());
        self.answer(result)
    }

    /// `fmi3GetShiftDecimal`. fmite's clocks start at the start time, with no shift.
    pub fn shifts(&mut self, vrs: &[u32], out: &mut [f64]) -> Status {
        let result = self.clock_values(vrs, out, |_| 0.0);
        self.answer(result)
    }

    fn clock_values(
        &self,
        vrs: &[u32],
        out: &mut [f64],
        value: fn(crate::Schedule) -> f64,
    ) -> Result<(), Error> {
        if !matches!(
            self.mode,
            Mode::Initialization { .. } | Mode::ClockActivation { .. }
        ) {
            return Err(self.not_in("fmi3GetInterval and fmi3GetShift"));
        }
        if vrs.len() != out.len() {
            return Err(Error::new("one value per clock"));
        }
        for (&vr, slot) in vrs.iter().zip(out) {
            let schedule = Self::clock(vr)?.clock.expect("a clock has a schedule");
            *slot = value(schedule);
        }
        Ok(())
    }
}

impl<T: ScheduledExecution> Instance<T> {
    /// `fmi3ActivateModelPartition`. Each clock's activations must come at strictly
    /// increasing times (FMI 3.0 §5.2.1). Clocks are not ordered among themselves.
    pub fn activate(&mut self, vr: u32, time: f64) -> Status {
        let result = self.try_activate(vr, time);
        self.answer(result)
    }

    fn try_activate(&mut self, vr: u32, time: f64) -> Result<(), Error> {
        let Mode::ClockActivation { now } = self.mode else {
            return Err(self.not_in("fmi3ActivateModelPartition"));
        };
        let clock = Self::clock(vr)?;
        if time.is_nan() {
            return Err(Error::new("the activation time is NaN"));
        }
        if let Some((_, before)) = self.activations.iter().find(|(c, _)| *c == vr)
            && time <= *before
        {
            return Err(Error::new(format!(
                "{} was activated at {before}, so {time} is not later",
                clock.name
            )));
        }
        (self.model).activate(Activation {
            clock: clock.clock.expect("a clock has a schedule"),
            time,
        })?;
        match self.activations.iter_mut().find(|(c, _)| *c == vr) {
            Some(last) => last.1 = time,
            None => self.activations.push((vr, time)),
        }
        self.set_since_activation = false;
        self.mode = Mode::ClockActivation { now: now.max(time) };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_model::{Gain, Ticker};

    fn context() -> Instantiation {
        Instantiation {
            instance_name: "test".to_owned(),
            resource_path: None,
        }
    }

    fn stepping() -> Instance<Gain> {
        let mut instance = Instance::instantiate(
            "{gain}",
            context(),
            Interface::CoSimulation,
            Logger::silent(),
        )
        .unwrap();
        assert_eq!(instance.set(&[2], Values::Float64(&[3.0])), Status::Ok);
        assert_eq!(instance.enter_initialization(0.0, None), Status::Ok);
        assert_eq!(instance.exit_initialization(), Status::Ok);
        instance
    }

    fn get_f64<const N: usize>(instance: &mut Instance<Gain>, vrs: &[u32]) -> [f64; N] {
        let mut out = [0.0; N];
        assert_eq!(instance.get(vrs, ValuesMut::Float64(&mut out)), Status::Ok);
        out
    }

    #[test]
    fn a_wrong_token_refuses_instantiation() {
        assert!(
            Instance::<Gain>::instantiate(
                "{other}",
                context(),
                Interface::CoSimulation,
                Logger::silent()
            )
            .is_err()
        );
    }

    #[test]
    fn a_step_reads_inputs_and_the_calculated_parameter() {
        let mut instance = stepping();
        assert_eq!(instance.set(&[1], Values::Float64(&[2.0])), Status::Ok);
        assert_eq!(instance.do_step(0.0, 0.1), (Status::Ok, false, 0.1));
        assert_eq!(
            get_f64::<4>(&mut instance, &[0, 3, 4]),
            [0.1, 6.0, 12.0, -2.0]
        );
    }

    #[test]
    fn a_tunable_parameter_set_in_step_mode_takes_effect_at_the_next_step() {
        let mut instance = stepping();
        assert_eq!(instance.set(&[2], Values::Float64(&[5.0])), Status::Ok);
        assert_eq!(get_f64::<1>(&mut instance, &[3]), [6.0]);
        assert_eq!(instance.do_step(0.0, 0.1).0, Status::Ok);
        assert_eq!(get_f64::<1>(&mut instance, &[3]), [10.0]);
    }

    #[test]
    fn time_counts_ticks_and_refuses_a_gap_or_a_fraction() {
        let mut instance = stepping();
        for i in 0..10 {
            let current = f64::from(i) * 0.1;
            assert_eq!(instance.do_step(current, 0.1).0, Status::Ok);
        }
        assert_eq!(get_f64::<1>(&mut instance, &[0]), [1.0]);
        assert_eq!(instance.do_step(1.5, 0.1).0, Status::Error);
        assert_eq!(instance.do_step(1.0, 0.05).0, Status::Error);
        assert_eq!(get_f64::<1>(&mut instance, &[0]), [1.0]);
    }

    #[test]
    fn the_state_machine_refuses_calls_out_of_order() {
        let mut instance = Instance::<Gain>::instantiate(
            "{gain}",
            context(),
            Interface::CoSimulation,
            Logger::silent(),
        )
        .unwrap();
        assert_eq!(instance.do_step(0.0, 0.1).0, Status::Error);
        assert_eq!(instance.exit_initialization(), Status::Error);
        assert_eq!(instance.terminate(), Status::Error);
        let mut out = [0.0];
        assert_eq!(
            instance.get(&[1], ValuesMut::Float64(&mut out)),
            Status::Error
        );
    }

    #[test]
    fn the_variable_table_refuses_bad_references_types_and_lengths() {
        let mut instance = stepping();
        let mut one = [0.0];
        let mut three = [0.0; 3];
        assert_eq!(
            instance.get(&[9], ValuesMut::Float64(&mut one)),
            Status::Error
        );
        assert_eq!(
            instance.get(&[5], ValuesMut::Float64(&mut one)),
            Status::Error
        );
        assert_eq!(
            instance.get(&[4], ValuesMut::Float64(&mut one)),
            Status::Error
        );
        assert_eq!(
            instance.get(&[4], ValuesMut::Float64(&mut three)),
            Status::Error
        );
        let mut steps = [7];
        assert_eq!(
            instance.get(&[5], ValuesMut::UInt32(&mut steps)),
            Status::Ok
        );
        assert_eq!(steps, [0]);
    }

    #[test]
    fn settability_follows_the_table() {
        let mut instance = stepping();
        // An output, a calculated parameter and `time`: the importer cannot set them.
        assert_eq!(instance.set(&[3], Values::Float64(&[1.0])), Status::Error);
        assert_eq!(instance.set(&[5], Values::UInt32(&[1])), Status::Error);
        assert_eq!(instance.set(&[0], Values::Float64(&[1.0])), Status::Error);
        // An input and a tunable parameter: settable in Step Mode.
        assert_eq!(
            instance.set(&[1, 2], Values::Float64(&[1.0, 2.0])),
            Status::Ok
        );
    }

    #[test]
    fn a_saved_state_restores_the_model_and_the_time() {
        let mut instance = stepping();
        instance.set(&[1], Values::Float64(&[1.0]));
        let saved = instance.save().ok().unwrap();
        instance.do_step(0.0, 0.1);
        instance.do_step(0.1, 0.1);
        assert_eq!(instance.restore(&saved), Status::Ok);
        assert_eq!(get_f64::<1>(&mut instance, &[0]), [0.0]);
        assert_eq!(*instance.model().steps, 0);
        assert_eq!(instance.do_step(0.0, 0.1).0, Status::Ok);
    }

    #[test]
    fn reset_instantiates_the_model_again() {
        let mut instance = stepping();
        instance.do_step(0.0, 0.1);
        assert_eq!(instance.reset(), Status::Ok);
        assert_eq!(*instance.model().k, 0.0);
        assert_eq!(instance.enter_initialization(0.0, None), Status::Ok);
    }

    #[test]
    fn debug_logging_accepts_only_the_declared_categories() {
        let mut instance = stepping();
        assert_eq!(instance.set_debug_logging(&[]), Status::Ok);
        assert_eq!(instance.set_debug_logging(&[log::ERROR]), Status::Ok);
        assert_eq!(instance.set_debug_logging(&["logEvents"]), Status::Error);
    }

    fn ticking() -> Instance<Ticker> {
        let mut instance = Instance::instantiate(
            "{ticker}",
            context(),
            Interface::ScheduledExecution,
            Logger::silent(),
        )
        .unwrap();
        assert_eq!(instance.enter_initialization(0.0, None), Status::Ok);
        assert_eq!(instance.exit_initialization(), Status::Ok);
        instance
    }

    fn get_ticker(instance: &mut Instance<Ticker>, vr: u32) -> f64 {
        let mut out = [0.0];
        assert_eq!(
            instance.get(&[vr], ValuesMut::Float64(&mut out)),
            Status::Ok
        );
        out[0]
    }

    #[test]
    fn each_clock_runs_its_own_partition() {
        let mut instance = ticking();
        assert_eq!(instance.set(&[3], Values::Float64(&[2.0])), Status::Ok);
        // Fastest first: the fast partition reads what the slow one held before.
        assert_eq!(instance.activate(1, 0.0), Status::Ok);
        assert_eq!(instance.activate(2, 0.0), Status::Ok);
        assert_eq!(get_ticker(&mut instance, 4), 2.0);
        assert_eq!(get_ticker(&mut instance, 5), 2.0);
        assert_eq!(instance.activate(1, 0.01), Status::Ok);
        assert_eq!(get_ticker(&mut instance, 4), 4.0);
        assert_eq!(get_ticker(&mut instance, 0), 0.01);
        assert_eq!(*instance.model().ticks, 3);
    }

    #[test]
    fn a_clock_moves_forward_and_only_a_clock_is_activated() {
        let mut instance = ticking();
        assert_eq!(instance.activate(1, 0.01), Status::Ok);
        // Clocks are not ordered among themselves.
        assert_eq!(instance.activate(2, 0.0), Status::Ok);
        assert_eq!(instance.activate(1, 0.01), Status::Error);
        let mut instance = ticking();
        assert_eq!(instance.activate(3, 0.0), Status::Error);
        let mut instance = ticking();
        assert_eq!(instance.activate(1, f64::NAN), Status::Error);
    }

    #[test]
    fn a_get_after_a_set_waits_for_an_activation() {
        let mut instance = ticking();
        assert_eq!(instance.set(&[3], Values::Float64(&[1.0])), Status::Ok);
        let mut out = [0.0];
        assert_eq!(
            instance.get(&[4], ValuesMut::Float64(&mut out)),
            Status::Error
        );
    }

    #[test]
    fn an_error_fails_every_call_until_terminate_or_reset() {
        let mut instance = ticking();
        assert_eq!(instance.activate(3, 0.0), Status::Error);
        assert_eq!(instance.activate(1, 0.0), Status::Error);
        assert_eq!(instance.set(&[3], Values::Float64(&[1.0])), Status::Error);
        assert_eq!(instance.terminate(), Status::Ok);
        assert_eq!(instance.reset(), Status::Ok);
        assert_eq!(instance.enter_initialization(0.0, None), Status::Ok);
    }

    #[test]
    fn a_clock_answers_its_interval_and_no_shift() {
        let mut instance = ticking();
        let mut out = [0.0; 2];
        assert_eq!(instance.intervals(&[1, 2], &mut out), Status::Ok);
        assert_eq!(out, [0.01, 0.05]);
        assert_eq!(instance.shifts(&[1, 2], &mut out), Status::Ok);
        assert_eq!(out, [0.0, 0.0]);
        assert_eq!(instance.intervals(&[3], &mut out[..1]), Status::Error);
    }

    #[test]
    fn a_clock_is_neither_got_nor_set() {
        let mut instance = ticking();
        let mut out = [0.0];
        assert_eq!(
            instance.get(&[1], ValuesMut::Float64(&mut out)),
            Status::Error
        );
    }

    #[test]
    fn co_simulation_activates_no_partition() {
        let mut instance = Instance::<Ticker>::instantiate(
            "{ticker}",
            context(),
            Interface::CoSimulation,
            Logger::silent(),
        )
        .unwrap();
        assert_eq!(instance.enter_initialization(0.0, None), Status::Ok);
        assert_eq!(instance.exit_initialization(), Status::Ok);
        assert_eq!(instance.activate(1, 0.0), Status::Error);
    }
}
