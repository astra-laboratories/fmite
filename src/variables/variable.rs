//! `Variable`, one entry of `<ModelVariables>`, and `Definition`, what a field type
//! says about the variable it declares.

use super::causality::Causality;
use super::unit::{Unit, UnitOf};
use super::{
    Dims, EnumerationType, Field, FmiType, Initial, InitialFor, Kind, VariabilityFor, VariabilityOf,
};
use crate::Error;

/// What a field type says about the variable it declares: its FMI type and shape, its
/// row of Table 22, read as attribute text and as the answers the instance needs, and
/// its unit.
pub trait Definition {
    const KIND: Kind;
    const DIMS: Dims;
    const ENUMERATION: Option<&'static EnumerationType>;
    const CAUSALITY: &'static str;
    const VARIABILITY: &'static str;
    const INITIAL: &'static str;
    /// The row's default initial. The model description leaves `initial` out when it
    /// is this, and for an input it must: the standard allows no `initial` there.
    const DEFAULT_INITIAL: &'static str;
    const HAS_START: bool;
    const SETTABLE_IN_INITIALIZATION: bool;
    const SETTABLE_IN_STEP: bool;
    const UNIT: Option<&'static (&'static str, Unit)>;
}

impl<C, T, U, V, I> Definition for Field<C, T, U, V, I>
where
    C: Causality,
    T: FmiType,
    U: UnitOf<T>,
    V: VariabilityFor<C> + VariabilityOf<T>,
    I: InitialFor<C, V>,
{
    const KIND: Kind = T::KIND;
    const DIMS: Dims = T::DIMS;
    const ENUMERATION: Option<&'static EnumerationType> = T::ENUMERATION;
    const CAUSALITY: &'static str = C::NAME;
    const VARIABILITY: &'static str = V::NAME;
    const INITIAL: &'static str = I::NAME;
    const DEFAULT_INITIAL: &'static str = V::DefaultInitial::NAME;
    const HAS_START: bool = I::HAS_START;
    const SETTABLE_IN_INITIALIZATION: bool = V::INITIALIZATION && I::HAS_START;
    const SETTABLE_IN_STEP: bool = V::STEP;
    const UNIT: Option<&'static (&'static str, Unit)> = U::DECLARED;
}

/// One entry of `<ModelVariables>`. `causality`, `variability` and `initial` are the
/// attribute text, for the model description only; the instance reads the answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Variable {
    pub name: &'static str,
    pub value_reference: u32,
    pub kind: Kind,
    pub dims: Dims,
    pub enumeration: Option<&'static EnumerationType>,
    pub causality: &'static str,
    pub variability: &'static str,
    pub initial: &'static str,
    pub default_initial: &'static str,
    pub has_start: bool,
    pub settable_in_initialization: bool,
    pub settable_in_step: bool,
    pub unit: Option<&'static (&'static str, Unit)>,
}

impl Variable {
    /// `Variable::new::<Output<f64>>("y", 1)` describes a field of that type.
    #[must_use]
    pub const fn new<F: Definition>(name: &'static str, value_reference: u32) -> Self {
        Self {
            name,
            value_reference,
            kind: F::KIND,
            dims: F::DIMS,
            enumeration: F::ENUMERATION,
            causality: F::CAUSALITY,
            variability: F::VARIABILITY,
            initial: F::INITIAL,
            default_initial: F::DEFAULT_INITIAL,
            has_start: F::HAS_START,
            settable_in_initialization: F::SETTABLE_IN_INITIALIZATION,
            settable_in_step: F::SETTABLE_IN_STEP,
            unit: F::UNIT,
        }
    }

    /// The number of values this variable takes from a call carrying `carried` values,
    /// `left` of them still unread.
    ///
    /// # Errors
    ///
    /// When the call's type is not this variable's, or fewer than its count are left.
    pub fn values_in(&self, carried: &str, left: usize) -> Result<usize, Error> {
        if carried != self.kind.carrier() {
            return Err(Error::new(format!(
                "{} is a {} variable, not {carried}",
                self.name,
                self.kind.name()
            )));
        }
        let count = self.dims.count();
        if count > left {
            return Err(Error::new(format!(
                "too few values for {}, which has {count}",
                self.name
            )));
        }
        Ok(count)
    }
}

/// Refuses, at compile time when called in `const`, a variable list the model
/// description could not state: a value reference that is 0, which is `time`'s, or is
/// used twice; a name used twice; two units of one name with different definitions;
/// two enumerations of one name with different items.
///
/// # Panics
///
/// On the first of those it finds, naming it.
pub const fn check(variables: &[Variable]) {
    let mut i = 0;
    while i < variables.len() {
        let a = &variables[i];
        assert!(a.value_reference != 0, "value reference 0 is `time`'s");
        let mut j = i + 1;
        while j < variables.len() {
            let b = &variables[j];
            assert!(
                a.value_reference != b.value_reference,
                "two variables share a value reference"
            );
            assert!(!same_str(a.name, b.name), "two variables share a name");
            if let (Some(x), Some(y)) = (a.unit, b.unit) {
                assert!(
                    !same_str(x.0, y.0) || same_unit(&x.1, &y.1),
                    "two units share a name and differ in definition"
                );
            }
            if let (Some(x), Some(y)) = (a.enumeration, b.enumeration) {
                assert!(
                    !same_str(x.name, y.name) || same_items(x.items, y.items),
                    "two enumerations share a name and differ in items"
                );
            }
            j += 1;
        }
        i += 1;
    }
}

const fn same_str(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

const fn same_unit(a: &Unit, b: &Unit) -> bool {
    a.kilogram == b.kilogram
        && a.meter == b.meter
        && a.second == b.second
        && a.ampere == b.ampere
        && a.kelvin == b.kelvin
        && a.mole == b.mole
        && a.candela == b.candela
        && a.radian == b.radian
        && a.factor.to_bits() == b.factor.to_bits()
        && a.offset.to_bits() == b.offset.to_bits()
}

const fn same_items(a: &[(&str, i64)], b: &[(&str, i64)]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if !same_str(a[i].0, b[i].0) || a[i].1 != b[i].1 {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::{Celsius, Unit};
    use crate::{
        CalculatedParameter, Constant, Discrete, Exact, Fixed, Input, Local, Output, Parameter,
        Tunable,
    };

    fn attributes<F: Definition>() -> (&'static str, &'static str, &'static str) {
        let variable = Variable::new::<F>("x", 0);
        (variable.causality, variable.variability, variable.initial)
    }

    fn settable<F: Definition>() -> (bool, bool) {
        let variable = Variable::new::<F>("x", 0);
        (
            variable.settable_in_initialization,
            variable.settable_in_step,
        )
    }

    #[test]
    fn defaults_follow_the_value_type() {
        assert_eq!(attributes::<Input<f64>>(), ("input", "continuous", "exact"));
        assert_eq!(attributes::<Input<i32>>(), ("input", "discrete", "exact"));
        assert_eq!(attributes::<Input<bool>>(), ("input", "discrete", "exact"));
        assert_eq!(
            attributes::<Input<[f32; 3]>>(),
            ("input", "continuous", "exact")
        );
        assert_eq!(
            attributes::<Output<f64>>(),
            ("output", "continuous", "calculated")
        );
        assert_eq!(
            attributes::<Output<u8>>(),
            ("output", "discrete", "calculated")
        );
        assert_eq!(
            attributes::<Local<f64>>(),
            ("local", "continuous", "calculated")
        );
    }

    #[test]
    fn parameters_default_to_fixed() {
        assert_eq!(
            attributes::<Parameter<f64>>(),
            ("parameter", "fixed", "exact")
        );
        assert_eq!(
            attributes::<Parameter<f64, (), Tunable>>(),
            ("parameter", "tunable", "exact")
        );
        assert_eq!(
            attributes::<CalculatedParameter<f64>>(),
            ("calculatedParameter", "fixed", "calculated")
        );
    }

    #[test]
    fn initial_defaults_to_the_first_cell_and_can_be_chosen() {
        assert_eq!(
            attributes::<Output<f64, (), Constant>>(),
            ("output", "constant", "exact")
        );
        assert_eq!(
            attributes::<Output<f64, (), Discrete, Exact>>(),
            ("output", "discrete", "exact")
        );
        assert_eq!(
            attributes::<Local<f64, (), Tunable>>(),
            ("local", "tunable", "calculated")
        );
    }

    #[test]
    fn a_start_value_follows_initial() {
        assert!(Variable::new::<Output<f64, (), Constant>>("x", 0).has_start);
        assert!(Variable::new::<Output<f64, (), Discrete, Exact>>("x", 0).has_start);
        assert!(!Variable::new::<Output<f64>>("x", 0).has_start);
    }

    #[test]
    fn the_host_sets_inputs_and_tunable_parameters_in_step_mode() {
        assert_eq!(settable::<Input<f64>>(), (true, true));
        assert_eq!(settable::<Parameter<f64, (), Tunable>>(), (true, true));
        assert_eq!(settable::<Parameter<f64>>(), (true, false));
        assert_eq!(
            settable::<CalculatedParameter<f64, (), Tunable>>(),
            (false, false)
        );
    }

    #[test]
    fn the_host_sets_a_start_value_unless_it_is_constant() {
        assert_eq!(
            settable::<Output<f64, (), Discrete, Exact>>(),
            (true, false)
        );
        assert_eq!(settable::<Output<f64>>(), (false, false));
        assert_eq!(settable::<Output<f64, (), Constant>>(), (false, false));
        assert_eq!(settable::<Local<f64, (), Constant>>(), (false, false));
        assert_eq!(settable::<Local<f64, (), Fixed>>(), (false, false));
    }

    #[test]
    fn a_unit_is_carried_by_floats_and_their_arrays() {
        let celsius = Some(&("degC", Unit::celsius()));
        assert_eq!(Variable::new::<Input<f64, Celsius>>("t", 1).unit, celsius);
        assert_eq!(
            Variable::new::<Output<[[f32; 2]; 2], Celsius>>("t", 1).unit,
            celsius
        );
        assert_eq!(Variable::new::<Output<f64>>("t", 1).unit, None);
        assert_eq!(Variable::new::<Output<i32>>("n", 1).unit, None);
    }

    #[test]
    fn a_variable_knows_its_type_and_shape() {
        let cells = Variable::new::<Output<[f64; 4], Celsius>>("cells", 1);
        assert_eq!(
            (cells.kind, cells.dims.as_slice()),
            (Kind::Float64, &[4][..])
        );
        assert_eq!(cells.enumeration, None);
    }

    #[test]
    fn a_consistent_list_passes_the_check() {
        const VARIABLES: &[Variable] = &[
            Variable::new::<Input<f64, Celsius>>("a", 1),
            Variable::new::<Output<f64, Celsius>>("b", 2),
        ];
        const { check(VARIABLES) };
    }

    #[test]
    #[should_panic(expected = "two units share a name and differ in definition")]
    fn two_units_of_one_name_are_refused() {
        struct Fake;
        impl crate::unit::UnitT for Fake {
            const NAME: &'static str = "degC";
            const UNIT: Unit = Unit::kelvin();
        }
        check(&[
            Variable::new::<Input<f64, Celsius>>("a", 1),
            Variable::new::<Output<f64, Fake>>("b", 2),
        ]);
    }

    #[test]
    #[should_panic(expected = "two variables share a value reference")]
    fn a_shared_value_reference_is_refused() {
        check(&[
            Variable::new::<Input<f64>>("a", 1),
            Variable::new::<Output<f64>>("b", 1),
        ]);
    }

    #[test]
    #[should_panic(expected = "value reference 0 is `time`'s")]
    fn value_reference_zero_is_refused() {
        check(&[Variable::new::<Input<f64>>("a", 0)]);
    }

    #[test]
    fn a_variable_is_buildable_in_const() {
        const VARIABLES: &[Variable] = &[
            Variable::new::<Output<f64, Celsius, Discrete>>("heater", 1),
            Variable::new::<Parameter<f64, (), Tunable>>("gain", 2),
        ];
        assert_eq!(VARIABLES[0].variability, "discrete");
        assert_eq!(VARIABLES[1].value_reference, 2);
    }
}
