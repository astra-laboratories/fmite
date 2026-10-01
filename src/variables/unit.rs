//! Units. FMI defines a unit by the exponents of eight base units and the map
//! `factor * value + offset` into them.

use super::{Float, FmiType};

/// The SI base units, plus the radian, which FMI counts as one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Kilogram,
    Metre,
    Second,
    Ampere,
    Kelvin,
    Mole,
    Candela,
    Radian,
}

/// The `<BaseUnit>` of a unit. Built in `const`:
/// `BaseUnit::ONE.with(Base::Metre, 1).with(Base::Second, -2)` is m/s².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseUnit {
    exponents: [i8; 8],
    pub factor: f64,
    pub offset: f64,
}

impl BaseUnit {
    pub const ONE: Self = Self {
        exponents: [0; 8],
        factor: 1.0,
        offset: 0.0,
    };

    /// Sets the exponent of `base`.
    #[must_use]
    pub const fn with(mut self, base: Base, exponent: i8) -> Self {
        self.exponents[base as usize] = exponent;
        self
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

    /// The product. An offset does not compose, so the result has none.
    #[must_use]
    pub const fn times(self, other: Self) -> Self {
        self.combine(other, 1, self.factor * other.factor)
    }

    /// The quotient. An offset does not compose, so the result has none.
    #[must_use]
    pub const fn per(self, other: Self) -> Self {
        self.combine(other, -1, self.factor / other.factor)
    }

    #[must_use]
    pub const fn exponent(self, base: Base) -> i8 {
        self.exponents[base as usize]
    }

    const fn combine(mut self, other: Self, sign: i8, factor: f64) -> Self {
        let mut i = 0;
        while i < self.exponents.len() {
            self.exponents[i] += sign * other.exponents[i];
            i += 1;
        }
        Self {
            factor,
            offset: 0.0,
            ..self
        }
    }
}

/// A unit, defined by the author. `NAME` is what the XML calls it.
pub trait Unit {
    const NAME: &'static str;
    const BASE: BaseUnit;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitDefinition {
    pub name: &'static str,
    pub base: BaseUnit,
}

/// `Self` is a legal unit for a variable of type `T`: `()` for none, or a [`Unit`] on a
/// float.
#[diagnostic::on_unimplemented(message = "a `{T}` variable cannot have unit `{Self}`")]
pub trait UnitOf<T> {
    const UNIT: Option<UnitDefinition>;
}

impl<T: FmiType> UnitOf<T> for () {
    const UNIT: Option<UnitDefinition> = None;
}

impl<U: Unit, T: Float> UnitOf<T> for U {
    const UNIT: Option<UnitDefinition> = Some(UnitDefinition {
        name: U::NAME,
        base: U::BASE,
    });
}

impl<U: Unit + UnitOf<T>, T, const N: usize> UnitOf<[T; N]> for U {
    const UNIT: Option<UnitDefinition> = <U as UnitOf<T>>::UNIT;
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRE: BaseUnit = BaseUnit::ONE.with(Base::Metre, 1);
    const SECOND: BaseUnit = BaseUnit::ONE.with(Base::Second, 1);
    const CELSIUS: BaseUnit = BaseUnit::ONE.with(Base::Kelvin, 1).offset(273.15);

    #[test]
    fn with_sets_one_exponent() {
        let newton = BaseUnit::ONE
            .with(Base::Kilogram, 1)
            .with(Base::Metre, 1)
            .with(Base::Second, -2);
        assert_eq!(newton.exponent(Base::Kilogram), 1);
        assert_eq!(newton.exponent(Base::Metre), 1);
        assert_eq!(newton.exponent(Base::Second), -2);
        assert_eq!(newton.exponent(Base::Kelvin), 0);
    }

    #[test]
    fn times_and_per_add_and_subtract_exponents() {
        let metre_second_squared = METRE.times(SECOND).times(SECOND);
        assert_eq!(
            metre_second_squared,
            BaseUnit::ONE.with(Base::Metre, 1).with(Base::Second, 2)
        );
        assert_eq!(
            METRE.per(SECOND).per(SECOND),
            BaseUnit::ONE.with(Base::Metre, 1).with(Base::Second, -2)
        );
    }

    #[test]
    fn factors_compose() {
        let km_per_hour = METRE.scaled(1000.0).per(SECOND.scaled(3600.0));
        assert!((km_per_hour.factor - 1000.0 / 3600.0).abs() < 1e-12);
    }

    #[test]
    fn composing_drops_the_offset() {
        let rate = CELSIUS.per(SECOND);
        assert_eq!(rate.offset, 0.0);
        assert_eq!(
            rate,
            BaseUnit::ONE.with(Base::Kelvin, 1).with(Base::Second, -1)
        );
    }
}
