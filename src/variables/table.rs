//! FMI 3.0 Table 22: the legal causality, variability and initial combinations.

use super::causality;
use super::{
    Calculated, Constant, Continuous, Discrete, Exact, Fixed, InitialMarker, Tunable,
    VariabilityMarker,
};

/// A row of the table: `Self` is a legal variability for causality `C`. `Initial` is
/// the default.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable cannot have variability `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait VariabilityFor<C>: VariabilityMarker {
    type Initial: InitialFor<C, Self>;
}

/// A cell of the table: `Self` is a legal initial for causality `C` with variability `V`.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable with variability `{V}` cannot have initial `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait InitialFor<C, V: ?Sized>: InitialMarker {}

/// `C` with variability `V` is computed by the model at every step, so model code may
/// write it.
pub trait Writable<V> {}

macro_rules! rows {
    ($($c:ident: $v:ident => $i:ident,)*) => {
        $(impl VariabilityFor<causality::$c> for $v { type Initial = $i; })*
    };
}

macro_rules! cells {
    ($($c:ident: $v:ident => $i:ident,)*) => {
        $(impl InitialFor<causality::$c, $v> for $i {})*
    };
}

rows! {
    Parameter: Fixed => Exact,
    Parameter: Tunable => Exact,
    CalculatedParameter: Fixed => Calculated,
    CalculatedParameter: Tunable => Calculated,
    Input: Discrete => Exact,
    Input: Continuous => Exact,
    Output: Constant => Exact,
    Output: Discrete => Calculated,
    Output: Continuous => Calculated,
    Local: Constant => Exact,
    Local: Fixed => Calculated,
    Local: Tunable => Calculated,
    Local: Discrete => Calculated,
    Local: Continuous => Calculated,
}

cells! {
    Parameter: Fixed => Exact,
    Parameter: Tunable => Exact,
    CalculatedParameter: Fixed => Calculated,
    CalculatedParameter: Tunable => Calculated,
    Input: Discrete => Exact,
    Input: Continuous => Exact,
    Output: Constant => Exact,
    Output: Discrete => Exact,
    Output: Discrete => Calculated,
    Output: Continuous => Exact,
    Output: Continuous => Calculated,
    Local: Constant => Exact,
    Local: Fixed => Calculated,
    Local: Tunable => Calculated,
    Local: Discrete => Exact,
    Local: Discrete => Calculated,
    Local: Continuous => Exact,
    Local: Continuous => Calculated,
}

impl Writable<Discrete> for causality::Output {}
impl Writable<Continuous> for causality::Output {}
impl Writable<Discrete> for causality::Local {}
impl Writable<Continuous> for causality::Local {}
