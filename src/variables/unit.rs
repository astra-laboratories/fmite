//! Units. FMI defines a unit by the exponents of eight base units and the map
//! `factor * value + offset`. A variable's unit is part of its type:
//! `Input<f64, unit::Volt>`.
//!
//! ```
//! use fmite::unit::{Unit, UnitT};
//!
//! struct KmPerHour;
//!
//! impl UnitT for KmPerHour {
//!     const NAME: &'static str = "km/h";
//!     const UNIT: Unit = Unit::meter().scaled(1000.0).per(Unit::second().scaled(3600.0));
//! }
//!
//! let _: fmite::Output<f64, KmPerHour>;
//! ```

use super::{Float, FmiType};

/// A unit: exponents of the SI base units and the radian (a base unit in FMI), plus
/// `factor` and `offset`. Each field is one `<BaseUnit>` attribute. Build one in `const`
/// from the units below: `Unit::meter().per(Unit::second().pow(2))` is m/s².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub kilogram: i8,
    pub meter: i8,
    pub second: i8,
    pub ampere: i8,
    pub kelvin: i8,
    pub mole: i8,
    pub candela: i8,
    pub radian: i8,
    pub factor: f64,
    pub offset: f64,
}

impl Unit {
    /// Dimensionless, every exponent zero.
    #[must_use]
    pub const fn one() -> Self {
        Self {
            kilogram: 0,
            meter: 0,
            second: 0,
            ampere: 0,
            kelvin: 0,
            mole: 0,
            candela: 0,
            radian: 0,
            factor: 1.0,
            offset: 0.0,
        }
    }

    #[must_use]
    pub const fn scaled(self, factor: f64) -> Self {
        Self {
            factor: self.factor * factor,
            ..self
        }
    }

    #[must_use]
    pub const fn offset(self, offset: f64) -> Self {
        Self { offset, ..self }
    }

    /// The product. Offsets do not compose, so the result has none.
    #[must_use]
    pub const fn times(self, other: Self) -> Self {
        self.combine(other, 1, self.factor * other.factor)
    }

    /// The quotient. Offsets do not compose, so the result has none.
    #[must_use]
    pub const fn per(self, other: Self) -> Self {
        self.combine(other, -1, self.factor / other.factor)
    }

    /// The `n`th power; a negative `n` gives the reciprocal. Built from
    /// [`times`](Self::times) or [`per`](Self::per), so it has no offset either.
    #[must_use]
    pub const fn pow(self, n: i8) -> Self {
        let mut power = Self::one();
        let mut i = 0;
        while i < n.unsigned_abs() {
            power = if n > 0 {
                power.times(self)
            } else {
                power.per(self)
            };
            i += 1;
        }
        power
    }

    /// Names every field instead of using `..self`, so a new base unit fails to compile
    /// until it is handled here.
    const fn combine(self, other: Self, sign: i8, factor: f64) -> Self {
        Self {
            kilogram: self.kilogram + sign * other.kilogram,
            meter: self.meter + sign * other.meter,
            second: self.second + sign * other.second,
            ampere: self.ampere + sign * other.ampere,
            kelvin: self.kelvin + sign * other.kelvin,
            mole: self.mole + sign * other.mole,
            candela: self.candela + sign * other.candela,
            radian: self.radian + sign * other.radian,
            factor,
            offset: 0.0,
        }
    }
}

/// A type that stands for a [`Unit`], so a variable can name its unit in its type.
/// Stable Rust allows no struct as a const generic, so the unit itself cannot be the
/// parameter. `NAME` is the unit's name in the model description.
pub trait UnitT {
    const NAME: &'static str;
    const UNIT: Unit;
}

/// `Self` is a valid unit for a variable of type `T`: `()` (no unit) on any type, or a
/// [`UnitT`] on a float. FMI 3.0 puts `unit` only in `fmi3RealBaseAttributes`, so
/// integer, boolean and string variables have no unit.
///
/// Like [`VariabilityOf`](super::VariabilityOf), only fmite implements it. Declare a
/// unit through [`UnitT`]. The two traits cannot merge: `()` would then be a `UnitT`,
/// and the float impl would overlap the `()` impl. `DECLARED` is `None` for `()`, and
/// otherwise the unit paired with its name. It is a reference, so every variable of
/// one unit shares the same pair.
#[diagnostic::on_unimplemented(message = "a `{T}` variable cannot have unit `{Self}`")]
pub trait UnitOf<T> {
    const DECLARED: Option<&'static (&'static str, Unit)>;
}

impl<T: FmiType> UnitOf<T> for () {
    const DECLARED: Option<&'static (&'static str, Unit)> = None;
}

// Scalars: only floats have units.
impl<U: UnitT, T: Float> UnitOf<T> for U {
    const DECLARED: Option<&'static (&'static str, Unit)> = Some(&(U::NAME, U::UNIT));
}

// Arrays: an array has its element's unit.
impl<U: UnitT + UnitOf<T>, T, const N: usize> UnitOf<[T; N]> for U {
    const DECLARED: Option<&'static (&'static str, Unit)> = <U as UnitOf<T>>::DECLARED;
}

/// Declares one unit per line: `Type, constructor = "name", unit;`. `Type` is what a
/// variable names, and `constructor` is the `const fn` on [`Unit`] that builds it.
macro_rules! units {
    ($($ty:ident, $constructor:ident = $name:literal, $unit:expr;)*) => {
        impl Unit {
            $(
                #[doc = concat!("`", $name, "`")]
                #[must_use]
                pub const fn $constructor() -> Self {
                    $unit
                }
            )*
        }

        $(
            #[doc = concat!("`", $name, "`")]
            #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
            pub struct $ty;

            impl UnitT for $ty {
                const NAME: &'static str = $name;
                const UNIT: Unit = Unit::$constructor();
            }
        )*
    };
}

units! {
    Kilogram, kilogram = "kg", Unit { kilogram: 1, ..Unit::one() };
    Meter, meter = "m", Unit { meter: 1, ..Unit::one() };
    Second, second = "s", Unit { second: 1, ..Unit::one() };
    Ampere, ampere = "A", Unit { ampere: 1, ..Unit::one() };
    Kelvin, kelvin = "K", Unit { kelvin: 1, ..Unit::one() };
    Mole, mole = "mol", Unit { mole: 1, ..Unit::one() };
    Candela, candela = "cd", Unit { candela: 1, ..Unit::one() };
    Radian, radian = "rad", Unit { radian: 1, ..Unit::one() };

    Hertz, hertz = "Hz", Unit::one().per(Unit::second());
    Newton, newton = "N", Unit::kilogram().times(Unit::meter()).per(Unit::second().pow(2));
    Pascal, pascal = "Pa", Unit::newton().per(Unit::meter().pow(2));
    Joule, joule = "J", Unit::newton().times(Unit::meter());
    Watt, watt = "W", Unit::joule().per(Unit::second());
    Coulomb, coulomb = "C", Unit::ampere().times(Unit::second());
    Volt, volt = "V", Unit::watt().per(Unit::ampere());
    Farad, farad = "F", Unit::coulomb().per(Unit::volt());
    Ohm, ohm = "Ohm", Unit::volt().per(Unit::ampere());
    Siemens, siemens = "S", Unit::one().per(Unit::ohm());
    Weber, weber = "Wb", Unit::volt().times(Unit::second());
    Tesla, tesla = "T", Unit::weber().per(Unit::meter().pow(2));
    Henry, henry = "H", Unit::weber().per(Unit::ampere());

    Celsius, celsius = "degC", Unit::kelvin().offset(273.15);
    AmpereHour, ampere_hour = "Ah", Unit::coulomb().scaled(3600.0);
    WattHour, watt_hour = "Wh", Unit::joule().scaled(3600.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_and_per_add_and_subtract_exponents() {
        let meter = Unit::meter();
        let second = Unit::second();
        assert_eq!(
            meter.times(second).times(second),
            Unit {
                meter: 1,
                second: 2,
                ..Unit::one()
            }
        );
        assert_eq!(
            meter.per(second).per(second),
            Unit {
                meter: 1,
                second: -2,
                ..Unit::one()
            }
        );
    }

    #[test]
    fn pow_repeats_times_or_per() {
        let km = Unit::meter().scaled(1000.0);
        assert_eq!(km.pow(2), km.times(km));
        assert_eq!(km.pow(-2), Unit::one().per(km).per(km));
        assert_eq!(km.pow(0), Unit::one());
        assert_eq!(Unit::celsius().pow(1), Unit::kelvin());
    }

    #[test]
    fn derived_units_have_their_si_exponents() {
        let volt = Unit {
            kilogram: 1,
            meter: 2,
            second: -3,
            ampere: -1,
            ..Unit::one()
        };
        assert_eq!(Unit::volt(), volt);
        assert_eq!(Unit::ohm(), Unit { ampere: -2, ..volt });
        assert_eq!(
            Unit::farad(),
            Unit {
                kilogram: -1,
                meter: -2,
                second: 4,
                ampere: 2,
                ..Unit::one()
            }
        );
        assert_eq!(Unit::siemens().times(Unit::ohm()), Unit::one());
        assert_eq!(Unit::hertz().radian, 0);
    }

    #[test]
    fn factors_compose() {
        let km_per_hour = Unit::meter()
            .scaled(1000.0)
            .per(Unit::second().scaled(3600.0));
        assert!((km_per_hour.factor - 1000.0 / 3600.0).abs() < 1e-12);
        assert_eq!(
            Unit::ampere_hour().per(Unit::coulomb()),
            Unit::one().scaled(3600.0)
        );
    }

    #[test]
    fn composing_drops_the_offset() {
        let rate = Unit::celsius().per(Unit::second());
        assert_eq!(rate.offset, 0.0);
        assert_eq!(
            rate,
            Unit {
                kelvin: 1,
                second: -1,
                ..Unit::one()
            }
        );
    }

    #[test]
    fn a_type_names_its_constructor() {
        assert_eq!(Celsius::UNIT, Unit::celsius());
        assert_eq!(WattHour::UNIT, Unit::watt_hour());
    }

    #[test]
    fn a_unit_is_legal_on_floats_and_their_arrays() {
        let celsius = Some(&("degC", Unit::celsius()));
        assert_eq!(<Celsius as UnitOf<f64>>::DECLARED, celsius);
        assert_eq!(<Celsius as UnitOf<[[f32; 2]; 2]>>::DECLARED, celsius);
        assert_eq!(<() as UnitOf<i32>>::DECLARED, None);
    }
}
