//! Model variables. Causality, variability, initial and unit are type parameters.
//! FMI 3.0 Table 22 is encoded once, in `table.rs`. The crate docs show, as
//! `compile_fail` tests, what an invalid combination or write looks like.

/// Implements a marker trait whose only item is its attribute text.
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
mod clock;
mod dims;
mod enumeration;
mod field;
mod fmi_type;
mod initial;
mod model_variables;
mod table;
pub mod unit;
mod variability;
mod variable;

pub use clock::*;
pub use dims::*;
pub use enumeration::*;
pub use field::*;
pub use fmi_type::*;
pub use initial::*;
pub use model_variables::*;
pub use table::*;
pub use variability::*;
pub use variable::*;
