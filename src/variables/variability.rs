//! Variability: when a variable may change.

use super::sealed::Sealed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variability {
    Constant,
    Fixed,
    Tunable,
    Discrete,
    Continuous,
}

pub trait VariabilityMarker: Sealed {
    const VARIABILITY: Variability;
}

pub struct Constant;
pub struct Fixed;
pub struct Tunable;
pub struct Discrete;
pub struct Continuous;

markers!(VariabilityMarker::VARIABILITY: Variability {
    Constant => Constant,
    Fixed => Fixed,
    Tunable => Tunable,
    Discrete => Discrete,
    Continuous => Continuous,
});
