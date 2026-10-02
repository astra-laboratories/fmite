//! Causality: who computes a variable. The markers live here, apart from the root,
//! because `Input` and `Output` also name the field aliases.

use crate::sealed::Sealed;

/// A causality marker. `NAME` is its `causality` attribute.
pub trait Causality: Sealed {
    const NAME: &'static str;
}

pub struct Parameter;
pub struct CalculatedParameter;
pub struct Input;
pub struct Output;
pub struct Local;

named!(Causality {
    Parameter => "parameter",
    CalculatedParameter => "calculatedParameter",
    Input => "input",
    Output => "output",
    Local => "local",
});
