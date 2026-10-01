//! Model variables. Causality, variability, initial and unit are type parameters, and
//! FMI 3.0 Table 22 is encoded once, in [`table`].
//!
//! An illegal combination does not compile:
//!
//! ```compile_fail
//! // A parameter is fixed or tunable.
//! let _: fmite::Parameter<f64, (), fmite::Continuous>;
//! ```
//!
//! ```compile_fail
//! // A constant output is exact.
//! let _: fmite::Output<f64, (), fmite::Constant, fmite::Calculated>;
//! ```
//!
//! ```compile_fail
//! // Only floats are continuous.
//! let _: fmite::Output<i32, (), fmite::Continuous>;
//! ```
//!
//! ```compile_fail
//! // Only floats have a unit.
//! struct Metre;
//! impl fmite::Unit for Metre {
//!     const NAME: &'static str = "m";
//!     const BASE: fmite::BaseUnit = fmite::BaseUnit::ONE.with(fmite::Base::Metre, 1);
//! }
//! let _: fmite::Output<i32, Metre>;
//! ```
//!
//! Model code cannot write an input, or an output that is constant:
//!
//! ```compile_fail
//! let mut u = fmite::Input::<f64>::new(0.0);
//! *u = 1.0;
//! ```
//!
//! ```compile_fail
//! let mut y = fmite::Output::<f64, (), fmite::Constant>::new(0.0);
//! *y = 1.0;
//! ```

mod sealed {
    pub trait Sealed {}
}

/// Implements a marker trait, mapping each marker type to its enum variant.
macro_rules! markers {
    ($trait:ident::$name:ident: $value:ident { $($ty:ty => $variant:ident,)* }) => {
        $(
            impl $crate::variables::sealed::Sealed for $ty {}
            impl $trait for $ty {
                const $name: $value = $value::$variant;
            }
        )*
    };
}

pub mod causality;
mod field;
mod fmi_type;
mod initial;
pub mod table;
mod unit;
mod variability;

pub use causality::{Causality, CausalityMarker};
pub use field::*;
pub use fmi_type::*;
pub use initial::*;
pub use table::{InitialFor, VariabilityFor, Writable};
pub use unit::*;
pub use variability::*;

/// What a [`Variable`] needs to know about a field type.
pub trait Describe {
    const CAUSALITY: Causality;
    const VARIABILITY: Variability;
    const INITIAL: Initial;
    const UNIT: Option<UnitDefinition>;
}

impl<C, T, U, V, I> Describe for Field<C, T, U, V, I>
where
    C: CausalityMarker,
    T: FmiType,
    U: UnitOf<T>,
    V: VariabilityMarker + VariabilityOf<T>,
    I: InitialFor<C, V>,
{
    const CAUSALITY: Causality = C::CAUSALITY;
    const VARIABILITY: Variability = V::VARIABILITY;
    const INITIAL: Initial = I::INITIAL;
    const UNIT: Option<UnitDefinition> = U::UNIT;
}

/// One entry of `<ModelVariables>`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    pub name: &'static str,
    pub value_reference: u32,
    pub causality: Causality,
    pub variability: Variability,
    pub initial: Initial,
    pub unit: Option<UnitDefinition>,
}

impl Variable {
    /// `Variable::new::<Output<f64>>("y", 1)` describes a field of that type.
    #[must_use]
    pub const fn new<F: Describe>(name: &'static str, value_reference: u32) -> Self {
        Self {
            name,
            value_reference,
            causality: F::CAUSALITY,
            variability: F::VARIABILITY,
            initial: F::INITIAL,
            unit: F::UNIT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Causality as C;
    use Initial as I;
    use Variability as V;

    struct Celsius;

    impl Unit for Celsius {
        const NAME: &'static str = "degC";
        const BASE: BaseUnit = BaseUnit::ONE.with(Base::Kelvin, 1).offset(273.15);
    }

    fn describe<F: Describe>() -> (Causality, Variability, Initial) {
        let v = Variable::new::<F>("x", 0);
        (v.causality, v.variability, v.initial)
    }

    #[test]
    fn defaults_follow_the_value_type() {
        assert_eq!(
            describe::<Input<f64>>(),
            (C::Input, V::Continuous, I::Exact)
        );
        assert_eq!(describe::<Input<i32>>(), (C::Input, V::Discrete, I::Exact));
        assert_eq!(describe::<Input<bool>>(), (C::Input, V::Discrete, I::Exact));
        assert_eq!(
            describe::<Input<[f32; 3]>>(),
            (C::Input, V::Continuous, I::Exact)
        );
        assert_eq!(
            describe::<Output<f64>>(),
            (C::Output, V::Continuous, I::Calculated)
        );
        assert_eq!(
            describe::<Output<u8>>(),
            (C::Output, V::Discrete, I::Calculated)
        );
        assert_eq!(
            describe::<Local<f64>>(),
            (C::Local, V::Continuous, I::Calculated)
        );
    }

    #[test]
    fn parameters_default_to_fixed() {
        assert_eq!(
            describe::<Parameter<f64>>(),
            (C::Parameter, V::Fixed, I::Exact)
        );
        assert_eq!(
            describe::<Parameter<f64, (), Tunable>>(),
            (C::Parameter, V::Tunable, I::Exact)
        );
        assert_eq!(
            describe::<CalculatedParameter<f64>>(),
            (C::CalculatedParameter, V::Fixed, I::Calculated)
        );
    }

    #[test]
    fn initial_defaults_to_the_first_cell_and_can_be_chosen() {
        assert_eq!(
            describe::<Output<f64, (), Constant>>(),
            (C::Output, V::Constant, I::Exact)
        );
        assert_eq!(
            describe::<Output<f64, (), Discrete, Exact>>(),
            (C::Output, V::Discrete, I::Exact)
        );
        assert_eq!(
            describe::<Local<f64, (), Tunable>>(),
            (C::Local, V::Tunable, I::Calculated)
        );
    }

    #[test]
    fn a_unit_is_carried_by_floats_and_their_arrays() {
        let celsius = Some(UnitDefinition {
            name: "degC",
            base: Celsius::BASE,
        });
        assert_eq!(Variable::new::<Input<f64, Celsius>>("t", 1).unit, celsius);
        assert_eq!(
            Variable::new::<Output<[[f32; 2]; 2], Celsius>>("t", 1).unit,
            celsius
        );
        assert_eq!(Variable::new::<Output<f64>>("t", 1).unit, None);
        assert_eq!(Variable::new::<Output<i32>>("n", 1).unit, None);
    }

    #[test]
    fn a_variable_is_buildable_in_const() {
        const VARIABLES: &[Variable] = &[
            Variable::new::<Output<f64, Celsius, Discrete>>("heater", 1),
            Variable::new::<Parameter<f64, (), Tunable>>("gain", 2),
        ];
        assert_eq!(VARIABLES[0].variability, Variability::Discrete);
        assert_eq!(VARIABLES[1].value_reference, 2);
    }
}
