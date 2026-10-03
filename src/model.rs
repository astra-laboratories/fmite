//! What an FMU implements: `Variables`, the core `Fmu`, the `CoSimulation` interface, and
//! the `State` capability.

use crate::{Error, Values, ValuesMut, Variable};

/// A value reference: how the importer names a variable. `0` is `time`, which fmite
/// declares itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueReference(pub u32);

impl ValueReference {
    /// The error a `get` or `set` answers for a reference it does not know. The
    /// instance looks every reference up first, so an implementation reaches this only
    /// when its `match` disagrees with its `VARIABLES`.
    #[must_use]
    pub fn unknown(self) -> Error {
        Error::new(format!("unknown value reference {}", self.0))
    }
}

/// The model's variables: the list the model description declares, and the table that
/// reads and writes them by value reference.
///
/// `#[derive(fmite::Variables)]` writes this from a struct's fields. A code generator,
/// or anyone who prefers it, writes it by hand, naming each field's type in
/// [`Variable::new`] so that FMI 3.0 Table 22 checks the list exactly as it checks a
/// field. Each `match` arm is one call on the field:
///
/// ```
/// use fmite::{Error, Input, Output, ValueReference, Values, ValuesMut, Variable};
///
/// #[derive(Default)]
/// struct Doubler {
///     u: Input<f64>,
///     y: Output<f64>,
/// }
///
/// impl fmite::Variables for Doubler {
///     const MODEL_NAME: &'static str = "doubler";
///     const INSTANTIATION_TOKEN: &'static str = "{doubler-1}";
///     const VARIABLES: &'static [Variable] = &[
///         Variable::new::<Input<f64>>("u", 1),
///         Variable::new::<Output<f64>>("y", 2),
///     ];
///
///     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
///         match vr.0 {
///             1 => self.u.get_into(out),
///             2 => self.y.get_into(out),
///             _ => Err(vr.unknown()),
///         }
///     }
///
///     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
///         match vr.0 {
///             1 => self.u.importer_set(values),
///             _ => Err(vr.unknown()),
///         }
///     }
/// }
/// ```
///
/// `set` lists only the variables the importer may set. The instance has already
/// refused the rest, by the `settable_in_*` answers in `VARIABLES`.
pub trait Variables {
    const MODEL_NAME: &'static str;
    const INSTANTIATION_TOKEN: &'static str;
    const VARIABLES: &'static [Variable];

    /// Writes the variable's values to `out`, which holds exactly as many as the
    /// variable has, in its FMI type.
    ///
    /// # Errors
    ///
    /// On an unknown reference, or values of the wrong type or count.
    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error>;

    /// Sets the variable from `values`, which hold exactly as many as the variable has,
    /// in its FMI type.
    ///
    /// # Errors
    ///
    /// On an unknown reference, or values of the wrong type or count, or an
    /// enumeration value with no item.
    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error>;
}

/// What the importer passes at instantiation. The instance keeps it, and `fmi3Reset`
/// instantiates the model from it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instantiation {
    pub instance_name: String,
    pub resource_path: Option<String>,
}

/// The `<DefaultExperiment>` the model description suggests to the importer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Experiment {
    pub start: Option<f64>,
    pub stop: Option<f64>,
    pub tolerance: Option<f64>,
    pub step: Option<f64>,
}

impl Experiment {
    pub const NONE: Self = Self {
        start: None,
        stop: None,
        tolerance: None,
        step: None,
    };
}

/// What every interface shares. Every method has a default, so a model that needs no
/// setup implements it in one line.
///
/// A calculated parameter, or a fixed or tunable local, is computed from the
/// parameters in `exit_initialization`, once the importer has set them. A tunable one
/// may be computed again in a step, since a tunable parameter may change between steps.
#[allow(
    unused_variables,
    reason = "defaults ignore what an override would read"
)]
pub trait Fmu: Variables + Default + Sized {
    const DESCRIPTION: Option<&'static str> = None;
    const DEFAULT_EXPERIMENT: Experiment = Experiment::NONE;

    /// Makes the model, at `fmi3InstantiateCoSimulation` and again at `fmi3Reset`.
    ///
    /// # Errors
    ///
    /// Refuses the instantiation; the importer gets a null instance, or `fmi3Error`
    /// from the reset.
    fn instantiate(context: &Instantiation) -> Result<Self, Error> {
        Ok(Self::default())
    }

    /// # Errors
    ///
    /// Refuses initialization with `fmi3Error`.
    fn enter_initialization(&mut self, start: f64, stop: Option<f64>) -> Result<(), Error> {
        Ok(())
    }

    /// The parameters are set; the model computes what follows from them.
    ///
    /// # Errors
    ///
    /// Refuses the end of initialization with `fmi3Error`.
    fn exit_initialization(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// # Errors
    ///
    /// Answers `fmi3Terminate` with `fmi3Error`.
    fn terminate(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// The Co-Simulation interface: the importer sets inputs, calls `do_step`, and reads
/// outputs, one communication step at a time.
pub trait CoSimulation: Fmu {
    /// The model's own step, when it has one. The instance then tracks time as
    /// `start + ticks × step`, and refuses a communication step that is not a whole
    /// number of them.
    const FIXED_INTERNAL_STEP_SIZE: Option<f64> = None;

    /// # Errors
    ///
    /// Answers `fmi3DoStep` with `fmi3Error`.
    fn do_step(&mut self, step: Step) -> Result<StepResult, Error>;
}

/// How much off a whole number of ticks a communication step may be, relative to it.
/// 0.1 s has no exact `f64`, so ten steps of it are not exactly 1.0 s; an importer's
/// arithmetic is off by a few ulps, and this allows it a margin some million times
/// wider while still refusing any step that is a real fraction of a tick.
pub const TICK_TOLERANCE: f64 = 1e-9;

/// One communication step: the importer's raw values, unrounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub current: f64,
    pub size: f64,
}

impl Step {
    /// The time the step ends at.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.current + self.size
    }

    /// The step as a whole number of `period`s.
    ///
    /// # Errors
    ///
    /// If the step is negative, or more than [`TICK_TOLERANCE`] off a whole number of
    /// periods.
    pub fn ticks(&self, period: f64) -> Result<u64, Error> {
        let exact = self.size / period;
        let rounded = exact.round();
        if rounded.is_nan()
            || rounded < 0.0
            || (exact - rounded).abs() > TICK_TOLERANCE * rounded.max(1.0)
        {
            return Err(Error::new(format!(
                "a step of {} is not a whole number of {period} ticks",
                self.size
            )));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "rounded and checked non-negative above; `as` saturates"
        )]
        Ok(rounded as u64)
    }
}

/// How a step ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepResult {
    /// The step reached its end.
    Complete,
    /// The step reached its end, and the model asks the importer to stop.
    Terminate,
}

/// The FMU state capability: the importer may save the instance and restore it. A
/// saved state is a typed copy, so any `Fmu` that is `Clone` has it.
pub trait State: Fmu + Clone {}

impl<T: Fmu + Clone> State for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(size: f64) -> Step {
        Step { current: 0.0, size }
    }

    #[test]
    fn a_step_is_a_whole_number_of_ticks() {
        assert_eq!(step(0.1).ticks(0.1), Ok(1));
        assert_eq!(step(0.3).ticks(0.1), Ok(3));
        assert_eq!(step(1.0 - 0.9).ticks(0.1), Ok(1));
        assert_eq!(step(0.0).ticks(0.1), Ok(0));
    }

    #[test]
    fn a_fraction_of_a_tick_is_refused() {
        assert!(step(0.15).ticks(0.1).is_err());
        assert!(step(-0.1).ticks(0.1).is_err());
        assert!(step(f64::NAN).ticks(0.1).is_err());
    }
}
