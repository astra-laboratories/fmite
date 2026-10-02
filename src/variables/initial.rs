//! Initial: whether a variable has a start value.

use crate::sealed::Sealed;

/// An initial marker. `NAME` is its `initial` attribute. `approx` is left out until
/// Model Exchange needs it.
pub trait Initial: Sealed {
    const NAME: &'static str;
    const HAS_START: bool;
}

pub struct Exact;
pub struct Calculated;

impl Sealed for Exact {}
impl Initial for Exact {
    const NAME: &'static str = "exact";
    const HAS_START: bool = true;
}

impl Sealed for Calculated {}
impl Initial for Calculated {
    const NAME: &'static str = "calculated";
    const HAS_START: bool = false;
}
