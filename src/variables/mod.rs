//! Model variables. Causality, variability, initial and unit are type parameters, and
//! FMI 3.0 Table 22 is encoded once, in `table.rs`.
//!
//! An illegal combination does not compile:
//!
//! ```compile_fail
//! // A parameter is fixed or tunable.
//! let _: fmite::Parameter<f64, (), fmite::Continuous>;
//! ```
//!
//! ```compile_fail
//! // A constant output is exact.
//! let _: fmite::Output<f64, (), fmite::Constant, fmite::Calculated>;
//! ```
//!
//! ```compile_fail
//! // Only floats are continuous.
//! let _: fmite::Output<i32, (), fmite::Continuous>;
//! ```
//!
//! ```compile_fail
//! // Only floats have a unit.
//! let _: fmite::Output<i32, fmite::unit::Meter>;
//! ```
//!
//! Model code cannot write an input, or an output that is constant:
//!
//! ```compile_fail
//! let mut u = fmite::Input::<f64>::new(0.0);
//! *u = 1.0;
//! ```
//!
//! ```compile_fail
//! let mut y = fmite::Output::<f64, (), fmite::Constant>::new(0.0);
//! *y = 1.0;
//! ```

/// Implements a marker trait whose one item is its attribute text.
macro_rules! named {
    ($trait:ident { $($ty:ty => $name:literal,)* }) => {
        $(
            impl $crate::sealed::Sealed for $ty {}
            impl $trait for $ty {
                const NAME: &'static str = $name;
            }
        )*
    };
}

pub mod causality;
mod enumeration;
mod field;
mod fmi_type;
mod initial;
mod table;
pub mod unit;
mod variability;
mod variable;

pub use enumeration::*;
pub use field::*;
pub use fmi_type::*;
pub use initial::*;
pub use table::*;
pub use variability::*;
pub use variable::*;
