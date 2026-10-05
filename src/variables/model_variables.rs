//! `Variables`, which declares a model's variables and reads and writes them by value
//! reference, and `check`, which rejects a list the model description cannot express.

use super::Variable;
use super::unit::Unit;
use crate::{Error, Values, ValuesMut};

/// A value reference: the number the importer uses for a variable. `0` is `time`,
/// which fmite declares itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueReference(pub u32);

impl ValueReference {
    /// The error for a reference `get` or `set` does not know. The instance checks every
    /// reference first, so this happens only when the `match` disagrees with
    /// `VARIABLES`.
    #[must_use]
    pub fn unknown(self) -> Error {
        Error::new(format!("unknown value reference {}", self.0))
    }
}

/// The model's variables: the list in the model description, and the lookup that reads
/// and writes them by value reference.
///
/// `#[derive(fmite::Variables)]` writes this from a struct's fields. To write it by
/// hand, name each field's type in [`Variable::new`], so FMI 3.0 Table 22 checks the
/// list the same way. Each `match` arm is one call on the field:
///
/// ```
/// use fmite::{Error, Input, Output, ValueReference, Values, ValuesMut, Variable};
///
/// #[derive(Default)]
/// struct Doubler {
///     u: Input<f64>,
///     y: Output<f64>,
/// }
///
/// impl fmite::Variables for Doubler {
///     const MODEL_NAME: &'static str = "doubler";
///     const INSTANTIATION_TOKEN: &'static str = "{doubler-1}";
///     const VARIABLES: &'static [Variable] = &[
///         Variable::new::<Input<f64>>("u", 1),
///         Variable::new::<Output<f64>>("y", 2),
///     ];
///
///     fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error> {
///         match vr.0 {
///             1 => self.u.get_into(out),
///             2 => self.y.get_into(out),
///             _ => Err(vr.unknown()),
///         }
///     }
///
///     fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error> {
///         match vr.0 {
///             1 => self.u.importer_set(values),
///             _ => Err(vr.unknown()),
///         }
///     }
/// }
/// ```
///
/// `set` lists only the variables the importer may set. The instance has already
/// rejected the rest, using the `settable_in_*` answers in `VARIABLES`.
pub trait Variables {
    const MODEL_NAME: &'static str;
    const INSTANTIATION_TOKEN: &'static str;
    const VARIABLES: &'static [Variable];

    /// Writes the variable's values to `out`, which has the variable's FMI type and
    /// exactly its number of values.
    ///
    /// # Errors
    ///
    /// On an unknown reference, or values of the wrong type or count.
    fn get(&self, vr: ValueReference, out: ValuesMut<'_>) -> Result<(), Error>;

    /// Sets the variable from `values`, which have the variable's FMI type and exactly
    /// its number of values.
    ///
    /// # Errors
    ///
    /// On an unknown reference, or values of the wrong type or count, or an
    /// enumeration value with no item.
    fn set(&mut self, vr: ValueReference, values: Values<'_>) -> Result<(), Error>;
}

/// Rejects a variable list the model description cannot express: value reference 0
/// (which is `time`'s), a value reference or name used twice, or two units or two
/// enumerations that share a name but differ. Called in `const`, it fails at compile
/// time.
///
/// # Panics
///
/// On the first problem it finds, naming it.
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
    use crate::unit::Celsius;
    use crate::{Input, Output};

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
}
