//! fmite - FMI 3.0 in Rust: implement a single trait for model and get the the FMU export for free.

#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::dbg_macro)]
#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]

pub mod abi;
pub mod description;
mod error;
pub mod export;
mod instance;
mod model;
#[cfg(feature = "package")]
pub mod package;
#[cfg(test)]
mod test_model;
mod values;
mod variables;

pub use error::Error;
#[cfg(feature = "derive")]
pub use fmite_derive::{Enumeration, Variables};
pub use instance::{Instance, Saved};
pub use model::*;
pub use values::{Carrier, Values, ValuesMut};
pub use variables::*;

/// Permission to write calculated variables: a `CalculatedParameter`, or a `Local` that
/// is `fixed` or `tunable`. The standard has the model compute them during
/// initialization and at no other time, so fmite hands this token to the
/// initialization hooks and to nothing else. Its field is private to this module, so
/// no code outside fmite can make one.
///
/// ```compile_fail
/// struct Model {
///     energy: fmite::CalculatedParameter<f64>,
/// }
///
/// impl Model {
///     // `do_step` holds no token, and there is no `DerefMut`.
///     fn do_step(&mut self) {
///         *self.energy = 1.0;
///     }
/// }
/// ```
///
/// ```compile_fail
/// let _ = fmite::Calculate(());
/// ```
pub struct Calculate(());

// Sealed trait for this library. No type outside of this crate can implement it, so any trait in
// this library that's protected by this, also cannot be implemented by types outside of this
// library.
mod sealed {
    pub trait Sealed {}

    /// Seals `FmiType` apart from `Sealed`: every `Enumeration` gets this one, and
    /// must not get `Sealed`, or an author's enum could implement `Float` or `Causality`.
    pub trait Value {}
}
