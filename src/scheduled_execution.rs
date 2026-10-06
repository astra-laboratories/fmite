//! Scheduled Execution: the importer is the scheduler. Each clock is a model partition,
//! and the importer runs one with `fmi3ActivateModelPartition` each time its clock
//! ticks.

use crate::{Error, Fmu, Periodic, Schedule};

/// The Scheduled Execution interface. The importer sets inputs, activates a clock's
/// partition, and reads outputs, one tick at a time. The model declares its clocks as
/// [`Clock`](crate::Clock) fields.
pub trait ScheduledExecution: Fmu {
    /// Runs the partition of the clock that ticked.
    ///
    /// # Errors
    ///
    /// `fmi3ActivateModelPartition` returns `fmi3Error`, and the instance refuses every
    /// call after it until `fmi3Terminate` or `fmi3Reset`.
    fn activate(&mut self, activation: Activation) -> Result<(), Error>;
}

/// One tick: which clock, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Activation {
    pub clock: Schedule,
    pub time: f64,
}

impl Activation {
    /// Whether the clock that ticked is `P`'s.
    #[must_use]
    pub const fn is<P: Periodic>(&self) -> bool {
        self.clock.is::<P>()
    }
}
