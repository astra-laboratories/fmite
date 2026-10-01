//! FMI 3.0 Table 22: the legal causality, variability and initial combinations, and
//! who may set or write a variable in each.

use super::causality::{self, Causality};
use super::{
    Calculated, Constant, Continuous, Discrete, Exact, Fixed, Initial, Tunable, Variability,
};

/// A row of the table: `Self` is a legal variability for causality `C`.
/// `DefaultInitial` is the first initial the row lists.
///
/// The two flags say what the host may set. In Instantiated and Initialization Mode,
/// a variable whose row is `INITIALIZATION` may be set if it has a start value. In
/// Step Mode, only inputs and tunable parameters may be.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable cannot have variability `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait VariabilityFor<C: Causality>: Variability {
    type DefaultInitial: InitialFor<C, Self>;
    const INITIALIZATION: bool;
    const STEP: bool;
}

/// A cell of the table: `Self` is a legal initial for causality `C` with variability `V`.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable with variability `{V}` cannot have initial `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait InitialFor<C, V: ?Sized>: Initial {}

/// `C` with variability `V` is computed by the model at every step, so model code may
/// write it.
pub trait Writable<V> {}

/// One line per row: `C: V => [default initial, other initials] flags`, where the
/// flags are `initialization`, `step` and `writable`.
macro_rules! table {
    ($($c:ident: $v:ident => [$default:ident $(, $other:ident)*] $($flag:ident)*;)*) => {
        $(
            impl VariabilityFor<causality::$c> for $v {
                type DefaultInitial = $default;
                const INITIALIZATION: bool = flag!(initialization in $($flag)*);
                const STEP: bool = flag!(step in $($flag)*);
            }
            impl InitialFor<causality::$c, $v> for $default {}
            $(impl InitialFor<causality::$c, $v> for $other {})*
            $(writable!($flag $c $v);)*
        )*
    };
}

/// Whether a row lists a flag.
macro_rules! flag {
    (initialization in initialization $($rest:ident)*) => { true };
    (step in step $($rest:ident)*) => { true };
    ($wanted:ident in $other:ident $($rest:ident)*) => { flag!($wanted in $($rest)*) };
    ($wanted:ident in) => { false };
}

/// The `Writable` impl for a row flagged `writable`.
macro_rules! writable {
    (writable $c:ident $v:ident) => {
        impl Writable<$v> for causality::$c {}
    };
    ($flag:ident $c:ident $v:ident) => {};
}

table! {
    Parameter: Fixed => [Exact] initialization;
    Parameter: Tunable => [Exact] initialization step;
    CalculatedParameter: Fixed => [Calculated];
    CalculatedParameter: Tunable => [Calculated];
    Input: Discrete => [Exact] initialization step;
    Input: Continuous => [Exact] initialization step;
    Output: Constant => [Exact];
    Output: Discrete => [Calculated, Exact] initialization writable;
    Output: Continuous => [Calculated, Exact] initialization writable;
    Local: Constant => [Exact];
    Local: Fixed => [Calculated];
    Local: Tunable => [Calculated];
    Local: Discrete => [Calculated, Exact] initialization writable;
    Local: Continuous => [Calculated, Exact] initialization writable;
}
