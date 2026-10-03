#![doc = include_str!("../docs/walkthrough.md")]
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

/// The README's example, compiled with the rest of the doctests.
#[cfg(all(doctest, feature = "derive"))]
#[doc = include_str!("../README.md")]
mod readme {}

/// The log categories fmite itself logs under, and the only ones an FMU declares: a
/// refused call is logged as an error, a panic as fatal. Each is a name and its
/// description, as `<LogCategories>` writes them.
pub const LOG_CATEGORIES: [(&str, &str); 2] = [
    ("logStatusError", "A refused call, and why"),
    (
        "logStatusFatal",
        "A panic inside the FMU; the instance takes no more calls",
    ),
];

// Sealed trait for this library. No type outside of this crate can implement it, so any trait in
// this library that's protected by this, also cannot be implemented by types outside of this
// library.
mod sealed {
    pub trait Sealed {}

    /// Seals `FmiType` apart from `Sealed`: every `Enumeration` gets this one, and
    /// must not get `Sealed`, or an author's enum could implement `Float` or `Causality`.
    pub trait Value {}
}
