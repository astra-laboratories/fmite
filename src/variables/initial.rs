//! Initial: whether a variable has a start value.

use super::sealed::Sealed;

/// `approx` is left out until Model Exchange needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initial {
    Exact,
    Calculated,
}

pub trait InitialMarker: Sealed {
    const INITIAL: Initial;
}

pub struct Exact;
pub struct Calculated;

markers!(InitialMarker::INITIAL: Initial {
    Exact => Exact,
    Calculated => Calculated,
});
