//! The Rust types that are FMI types.

use super::sealed::Sealed;
use super::{Constant, Continuous, Discrete, Fixed, Tunable, VariabilityMarker};

/// A Rust type that is an FMI type. `Variability` is the standard's default for it.
pub trait FmiType: Sealed {
    type Variability: VariabilityMarker;
}

/// `f32` and `f64`: the only types that can be continuous or carry a unit.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a float",
    note = "only floats are continuous or carry a unit"
)]
pub trait Float: FmiType {}

/// `Self` is a legal variability for a variable of type `T`.
#[diagnostic::on_unimplemented(message = "a `{T}` variable cannot be `{Self}`")]
pub trait VariabilityOf<T> {}

macro_rules! fmi_types {
    ($variability:ident: $($ty:ty),*) => {
        $(
            impl Sealed for $ty {}
            impl FmiType for $ty {
                type Variability = $variability;
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
    type Variability = T::Variability;
}

impl<T: FmiType> VariabilityOf<T> for Constant {}
impl<T: FmiType> VariabilityOf<T> for Fixed {}
impl<T: FmiType> VariabilityOf<T> for Tunable {}
impl<T: FmiType> VariabilityOf<T> for Discrete {}
impl<T: Float> VariabilityOf<T> for Continuous {}
impl<T, const N: usize> VariabilityOf<[T; N]> for Continuous where Self: VariabilityOf<T> {}
