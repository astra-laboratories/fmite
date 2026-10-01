//! fmite - FMI 3.0 in Rust: implement a single trait for model and get the the FMU export for free.

#![deny(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::dbg_macro)]
#![warn(clippy::pedantic)]
#![warn(unused_crate_dependencies)]

mod variable;

pub use variable::*;
