//! fmite - FMI 3.0 in Rust: implement a single trait for model and get the the FMU export for free.

#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::dbg_macro)]
#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]

mod variables;

pub use variables::*;

// Sealed trait for this library. No type outside of this crate can implement it, so any trait in
// this library that's protected by this, also cannot be implemented by types outside of this
// library.
mod sealed {
    pub trait Sealed {}
}
