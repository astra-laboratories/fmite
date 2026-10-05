//! Variability: when a variable may change, and which variabilities each value type
//! allows.

use super::{Float, FmiType};
use crate::sealed::Sealed;

/// A variability marker. `NAME` is its `variability` attribute.
pub trait Variability: Sealed {
    const NAME: &'static str;
}

pub struct Constant;
pub struct Fixed;
pub struct Tunable;
pub struct Discrete;
pub struct Continuous;

named!(Variability {
    Constant => "constant",
    Fixed => "fixed",
    Tunable => "tunable",
    Discrete => "discrete",
    Continuous => "continuous",
});

/// `Self` is a valid variability for a variable of type `T`. All are valid, except that
/// only floats can be continuous.
#[diagnostic::on_unimplemented(message = "a `{T}` variable cannot be `{Self}`")]
pub trait VariabilityOf<T> {}

impl<T: FmiType> VariabilityOf<T> for Constant {}
impl<T: FmiType> VariabilityOf<T> for Fixed {}
impl<T: FmiType> VariabilityOf<T> for Tunable {}
impl<T: FmiType> VariabilityOf<T> for Discrete {}
impl<T: Float> VariabilityOf<T> for Continuous {}
impl<T, const N: usize> VariabilityOf<[T; N]> for Continuous where Self: VariabilityOf<T> {}
