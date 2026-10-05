//! What every FMU implements, whichever interface it exports: the core `Fmu`, what it
//! is instantiated from, and the `State` capability.

use crate::{Error, Variables};

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

    /// Makes the model, at instantiation and again at `fmi3Reset`.
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

/// The FMU state capability: the importer may save the instance and restore it. A
/// saved state is a typed copy, so any `Fmu` that is `Clone` has it.
pub trait State: Fmu + Clone {}

impl<T: Fmu + Clone> State for T {}
