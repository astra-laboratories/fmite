//! Causality: who computes a variable. The markers live here, apart from the root,
//! because `Input` and `Output` also name the field aliases.

use super::sealed::Sealed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causality {
    Parameter,
    CalculatedParameter,
    Input,
    Output,
    Local,
}

pub trait CausalityMarker: Sealed {
    const CAUSALITY: Causality;
}

pub struct Parameter;
pub struct CalculatedParameter;
pub struct Input;
pub struct Output;
pub struct Local;

markers!(CausalityMarker::CAUSALITY: Causality {
    Parameter => Parameter,
    CalculatedParameter => CalculatedParameter,
    Input => Input,
    Output => Output,
    Local => Local,
});
