//! Co-Simulation: the interface, the communication step it advances by, and the clock
//! the instance keeps time with.

use crate::{Error, Fmu};

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

/// Time in Step Mode. With a fixed step the instance counts ticks and computes `now`
/// from them, so rounding does not accumulate over a long run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    start: f64,
    ticks: u64,
    pub now: f64,
}

impl Clock {
    #[must_use]
    pub fn starting(start: f64) -> Self {
        Self {
            start,
            ticks: 0,
            now: start,
        }
    }

    /// Refuses a step that does not start where the last one ended.
    ///
    /// # Errors
    ///
    /// When `current` is more than [`TICK_TOLERANCE`] off the time reached.
    pub fn check_start(&self, current: f64) -> Result<(), Error> {
        if (current - self.now).abs() > TICK_TOLERANCE * self.now.abs().max(1.0) {
            return Err(Error::new(format!(
                "the step starts at {current}, but the instance is at {}",
                self.now
            )));
        }
        Ok(())
    }

    /// Moves to the step's end: by whole ticks with a fixed `period`, else to `step.end()`.
    ///
    /// # Errors
    ///
    /// When the step is not a whole number of `period`s.
    #[expect(
        clippy::cast_precision_loss,
        reason = "ticks are far below 2^53 in any run"
    )]
    pub fn advance(&mut self, step: Step, period: Option<f64>) -> Result<(), Error> {
        match period {
            Some(period) => {
                self.ticks += step.ticks(period)?;
                self.now = self.start + self.ticks as f64 * period;
            }
            None => self.now = step.end(),
        }
        Ok(())
    }
}

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
