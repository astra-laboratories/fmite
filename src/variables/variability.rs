//! Variability: when a variable may change.

use super::sealed::Sealed;

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
