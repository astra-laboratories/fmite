//! `Instance<T>`: one instantiated FMU. It checks every call the importer makes against
//! the standard's state machine and against `T::VARIABLES`, so that `T` sees only the
//! calls the standard allows, with references it knows and values of the right type.

use crate::abi::{Logger, Status};
use crate::co_simulation::Clock;
use crate::values::surplus;
use crate::{
    CoSimulation, Error, Fmu, Instantiation, LOG_CATEGORIES, State, Step, StepResult,
    ValueReference, Values, ValuesMut, Variable, check,
};

/// Where the instance is in the Co-Simulation state machine. Each variant holds what its
/// mode can answer and nothing else: there is no time before initialization, and no
/// clock before Step Mode.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Instantiated,
    Initialization {
        start: f64,
    },
    Step(Clock),
    Terminated {
        now: f64,
    },
    /// A call panicked; the model may be half-updated, so only freeing is allowed.
    Fatal,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Instantiated => "Instantiated",
            Self::Initialization { .. } => "Initialization Mode",
            Self::Step(_) => "Step Mode",
            Self::Terminated { .. } => "Terminated",
            Self::Fatal => "an error state",
        }
    }
}

/// A saved FMU state: the model and the instance's own state with it.
pub struct Saved<T> {
    model: T,
    mode: Mode,
}

/// One instantiated FMU.
pub struct Instance<T: Fmu> {
    model: T,
    context: Instantiation,
    logger: Logger,
    mode: Mode,
}

impl<T: Fmu> Instance<T> {
    /// Checks the importer's token against `T`'s, and instantiates `T`.
    ///
    /// # Errors
    ///
    /// When the token differs or `T::instantiate` refuses; the error is logged too.
    pub fn instantiate(token: &str, context: Instantiation, logger: Logger) -> Result<Self, Error> {
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
            logger,
            mode: Mode::Instantiated,
        })
    }

    /// The model, for tests and for the description writer.
    pub fn model(&self) -> &T {
        &self.model
    }

    /// Logs an error and answers `fmi3Error`, or answers `fmi3OK`.
    fn answer(&self, result: Result<(), Error>) -> Status {
        match result {
            Ok(()) => Status::Ok,
            Err(error) => {
                self.logger.error(error.message());
                Status::Error
            }
        }
    }

    fn not_in(&self, function: &str) -> Error {
        Error::new(format!("{function} is not allowed in {}", self.mode.name()))
    }

    /// Answers a function this FMU does not implement.
    pub fn refuse(&self, function: &str, why: &str) -> Status {
        self.answer(Err(Error::new(format!("{function}: {why}"))))
    }

    /// Records a panic: logged as fatal, and the instance takes no more calls.
    pub fn poison(&mut self, function: &str, panic: &str) -> Status {
        self.mode = Mode::Fatal;
        self.logger.log(
            Status::Fatal,
            "logStatusFatal",
            &format!("{function} panicked: {panic}"),
        );
        Status::Fatal
    }

    /// `fmi3SetDebugLogging`. fmite has no debug messages, so the call only checks
    /// that every category is one the FMU declares.
    pub fn set_debug_logging(&mut self, categories: &[&str]) -> Status {
        let unknown = (categories.iter())
            .find(|category| !LOG_CATEGORIES.iter().any(|(name, _)| name == *category));
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
            Mode::Initialization { start } => self
                .model
                .exit_initialization()
                .map(|()| self.mode = Mode::Step(Clock::starting(start))),
            _ => Err(self.not_in("fmi3ExitInitializationMode")),
        };
        self.answer(result)
    }

    /// `fmi3Terminate`.
    pub fn terminate(&mut self) -> Status {
        let result = match self.mode {
            Mode::Step(clock) => self
                .model
                .terminate()
                .map(|()| self.mode = Mode::Terminated { now: clock.now }),
            _ => Err(self.not_in("fmi3Terminate")),
        };
        self.answer(result)
    }

    /// `fmi3Reset`: the model is instantiated again, from the same context.
    pub fn reset(&mut self) -> Status {
        let result = match self.mode {
            Mode::Fatal => Err(self.not_in("fmi3Reset")),
            _ => T::instantiate(&self.context).map(|model| {
                self.model = model;
                self.mode = Mode::Instantiated;
            }),
        };
        self.answer(result)
    }

    fn variable(vr: u32) -> Result<&'static Variable, Error> {
        (T::VARIABLES.iter())
            .find(|variable| variable.value_reference == vr)
            .ok_or_else(|| ValueReference(vr).unknown())
    }

    /// The current time, `time`'s value.
    fn now(&self) -> f64 {
        match self.mode {
            Mode::Initialization { start } => start,
            Mode::Step(clock) => clock.now,
            Mode::Terminated { now } => now,
            Mode::Instantiated | Mode::Fatal => 0.0,
        }
    }

    /// `fmi3Get{Type}`: `out` holds the values of every reference, in order.
    pub fn get(&mut self, vrs: &[u32], out: ValuesMut<'_>) -> Status {
        let result = self.try_get(vrs, out);
        self.answer(result)
    }

    fn try_get(&mut self, vrs: &[u32], mut out: ValuesMut<'_>) -> Result<(), Error> {
        match self.mode {
            Mode::Initialization { .. } | Mode::Step(_) | Mode::Terminated { .. } => {}
            Mode::Instantiated | Mode::Fatal => return Err(self.not_in("fmi3Get")),
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

    /// `fmi3Set{Type}`: `values` holds the values of every reference, in order.
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
                Mode::Step(_) => variable.settable_in_step,
                Mode::Terminated { .. } | Mode::Fatal => false,
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
        }
        surplus(values.len())
    }

    /// `fmi3GetFMUState`.
    ///
    /// # Errors
    ///
    /// Outside the modes a state can be taken in.
    pub fn save(&self) -> Result<Saved<T>, Status>
    where
        T: State,
    {
        match self.mode {
            Mode::Instantiated | Mode::Fatal => {
                Err(self.answer(Err(self.not_in("fmi3GetFMUState"))))
            }
            mode => Ok(Saved {
                model: self.model.clone(),
                mode,
            }),
        }
    }

    /// `fmi3SetFMUState`.
    pub fn restore(&mut self, saved: &Saved<T>) -> Status
    where
        T: State,
    {
        if self.mode == Mode::Fatal {
            return self.answer(Err(self.not_in("fmi3SetFMUState")));
        }
        self.model.clone_from(&saved.model);
        self.mode = saved.mode;
        Status::Ok
    }
}

impl<T: CoSimulation> Instance<T> {
    /// `fmi3DoStep`. Returns whether the model asks to terminate, and the time reached.
    pub fn do_step(&mut self, current: f64, size: f64) -> (Status, bool, f64) {
        let Mode::Step(mut clock) = self.mode else {
            let status = self.answer(Err(self.not_in("fmi3DoStep")));
            return (status, false, self.now());
        };
        let step = Step { current, size };
        let result = clock.check_start(current).and_then(|()| {
            let mut next = clock;
            next.advance(step, T::FIXED_INTERNAL_STEP_SIZE)?;
            let result = self.model.do_step(step)?;
            clock = next;
            Ok(result)
        });
        self.mode = Mode::Step(clock);
        match result {
            Ok(result) => (Status::Ok, result == StepResult::Terminate, clock.now),
            Err(error) => (self.answer(Err(error)), false, clock.now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_model::Gain;

    fn context() -> Instantiation {
        Instantiation {
            instance_name: "test".to_owned(),
            resource_path: None,
        }
    }

    fn stepping() -> Instance<Gain> {
        let mut instance = Instance::instantiate("{gain}", context(), Logger::silent()).unwrap();
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
        assert!(Instance::<Gain>::instantiate("{other}", context(), Logger::silent()).is_err());
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
        let mut instance =
            Instance::<Gain>::instantiate("{gain}", context(), Logger::silent()).unwrap();
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
        // An output, a calculated parameter: never set by the importer.
        assert_eq!(instance.set(&[3], Values::Float64(&[1.0])), Status::Error);
        assert_eq!(instance.set(&[5], Values::UInt32(&[1])), Status::Error);
        assert_eq!(instance.set(&[0], Values::Float64(&[1.0])), Status::Error);
        // An input and a tunable parameter: set in Step Mode.
        assert_eq!(
            instance.set(&[1, 2], Values::Float64(&[1.0, 2.0])),
            Status::Ok
        );
    }

    #[test]
    fn a_saved_state_restores_the_model_and_the_clock() {
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
        assert_eq!(instance.set_debug_logging(&["logStatusError"]), Status::Ok);
        assert_eq!(instance.set_debug_logging(&["logEvents"]), Status::Error);
    }
}
