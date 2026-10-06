// The walkthrough's examples use the derives, so it is the crate docs only with `derive`.
#![cfg_attr(feature = "derive", doc = include_str!("../docs/walkthrough.md"))]
#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::dbg_macro)]
#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]

pub mod abi;
mod co_simulation;
pub mod description;
mod error;
pub mod export;
mod fmu;
mod instance;
pub mod log;
#[cfg(feature = "package")]
pub mod package;
mod scheduled_execution;
#[cfg(test)]
mod test_model;
mod values;
mod variables;

pub use co_simulation::{CoSimulation, Step, StepResult, TICK_TOLERANCE};
pub use error::Error;
#[cfg(feature = "derive")]
pub use fmite_derive::{Enumeration, Variables};
pub use fmu::*;
pub use instance::{Instance, Saved};
pub use scheduled_execution::{Activation, ScheduledExecution};
pub use values::{Carrier, Values, ValuesMut};
pub use variables::*;

/// The README's example, compiled with the rest of the doctests.
#[cfg(all(doctest, feature = "derive"))]
#[doc = include_str!("../README.md")]
mod readme {}

// Sealed traits. Only this crate can implement them, so a trait that requires one cannot
// be implemented outside this crate either.
mod sealed {
    pub trait Sealed {}

    /// Seals `FmiType` separately from `Sealed`. Every `Enumeration` implements this one
    /// but not `Sealed`, or a user's enum could implement `Float` or `Causality`.
    pub trait Value {}
}
