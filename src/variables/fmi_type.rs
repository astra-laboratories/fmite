//! The Rust types that are FMI types.

use super::{Continuous, Discrete, Variability};
use crate::sealed::Sealed;

/// A Rust type that is an FMI type. `DefaultVariability` is the standard's default for it.
pub trait FmiType: Sealed {
    type DefaultVariability: Variability;
}

/// `f32` and `f64`: the only types that can be continuous or carry a unit.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a float",
    note = "only floats are continuous or carry a unit"
)]
pub trait Float: FmiType {}

macro_rules! fmi_types {
    ($variability:ident: $($ty:ty),*) => {
        $(
            impl Sealed for $ty {}
            impl FmiType for $ty {
                type DefaultVariability = $variability;
            }
        )*
    };
}

fmi_types!(Continuous: f32, f64);
fmi_types!(Discrete: i8, i16, i32, i64, u8, u16, u32, u64, bool);

impl Float for f32 {}
impl Float for f64 {}

impl<T: FmiType, const N: usize> Sealed for [T; N] {}
impl<T: FmiType, const N: usize> FmiType for [T; N] {
    type DefaultVariability = T::DefaultVariability;
}
