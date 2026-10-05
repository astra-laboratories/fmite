//! The Rust types that are FMI types, and how their values cross a get or set.

use super::Dims;
use super::enumeration::{Enumeration, EnumerationType};
use super::{Continuous, Discrete, Variability};
use crate::sealed::{Sealed, Value};
use crate::{Error, Values, ValuesMut};

/// The FMI type of a variable: the element `<ModelVariables>` writes for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Float32,
    Float64,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Boolean,
    Enumeration,
}

impl Kind {
    /// The element name, `"Float64"`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Float32 => "Float32",
            Self::Float64 => "Float64",
            Self::Int8 => "Int8",
            Self::UInt8 => "UInt8",
            Self::Int16 => "Int16",
            Self::UInt16 => "UInt16",
            Self::Int32 => "Int32",
            Self::UInt32 => "UInt32",
            Self::Int64 => "Int64",
            Self::UInt64 => "UInt64",
            Self::Boolean => "Boolean",
            Self::Enumeration => "Enumeration",
        }
    }

    /// The type whose get and set functions carry the values: the type itself, except
    /// that an enumeration travels as `Int64`.
    #[must_use]
    pub const fn carrier(self) -> &'static str {
        match self {
            Self::Enumeration => Self::Int64.name(),
            other => other.name(),
        }
    }
}

/// A Rust type that is an FMI type. `DefaultVariability` is the standard's default for
/// it. Implemented by fmite for the scalars and arrays of them, and for every
/// [`Enumeration`].
///
/// `read` and `write` move a value through a get or set buffer, scalars in row-major
/// order starting at `at`. They are for the instance and the `Variables` helpers on
/// `Field`; model code has no use for them.
pub trait FmiType: Value + Copy {
    type DefaultVariability: Variability;
    const KIND: Kind;
    const DIMS: Dims;
    const ENUMERATION: Option<&'static EnumerationType>;

    #[doc(hidden)]
    fn read(&self, out: &mut ValuesMut<'_>, at: &mut usize) -> Result<(), Error>;

    #[doc(hidden)]
    fn write(&mut self, values: &Values<'_>, at: &mut usize) -> Result<(), Error>;
}

/// `f32` and `f64`: the only types that can be continuous or carry a unit.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a float",
    note = "only floats are continuous or carry a unit"
)]
pub trait Float: FmiType + Sealed {}

fn mismatch(kind: Kind, carried: &str) -> Error {
    Error::new(format!(
        "a {} variable cannot take {carried} values",
        kind.name()
    ))
}

fn put<T>(out: &mut [T], at: &mut usize, value: T) -> Result<(), Error> {
    let slot = out
        .get_mut(*at)
        .ok_or_else(|| Error::new("too few values for the variable"))?;
    *slot = value;
    *at += 1;
    Ok(())
}

fn take<T: Copy>(values: &[T], at: &mut usize) -> Result<T, Error> {
    let value = values
        .get(*at)
        .copied()
        .ok_or_else(|| Error::new("too few values for the variable"))?;
    *at += 1;
    Ok(value)
}

macro_rules! scalars {
    ($variability:ident: $($ty:ty => $kind:ident),*) => {
        $(
            impl Sealed for $ty {}
            impl Value for $ty {}
            impl FmiType for $ty {
                type DefaultVariability = $variability;
                const KIND: Kind = Kind::$kind;
                const DIMS: Dims = Dims::SCALAR;
                const ENUMERATION: Option<&'static EnumerationType> = None;

                fn read(&self, out: &mut ValuesMut<'_>, at: &mut usize) -> Result<(), Error> {
                    match out {
                        ValuesMut::$kind(out) => put(out, at, *self),
                        other => Err(mismatch(Self::KIND, other.type_name())),
                    }
                }

                fn write(&mut self, values: &Values<'_>, at: &mut usize) -> Result<(), Error> {
                    match values {
                        Values::$kind(values) => {
                            *self = take(values, at)?;
                            Ok(())
                        }
                        other => Err(mismatch(Self::KIND, other.type_name())),
                    }
                }
            }
        )*
    };
}

scalars!(Continuous: f32 => Float32, f64 => Float64);
scalars!(Discrete:
    i8 => Int8, u8 => UInt8, i16 => Int16, u16 => UInt16,
    i32 => Int32, u32 => UInt32, i64 => Int64, u64 => UInt64,
    bool => Boolean
);

impl Float for f32 {}
impl Float for f64 {}

impl<T: FmiType, const N: usize> Value for [T; N] {}
impl<T: FmiType, const N: usize> FmiType for [T; N] {
    type DefaultVariability = T::DefaultVariability;
    const KIND: Kind = T::KIND;
    const DIMS: Dims = T::DIMS.outer(N);
    const ENUMERATION: Option<&'static EnumerationType> = T::ENUMERATION;

    fn read(&self, out: &mut ValuesMut<'_>, at: &mut usize) -> Result<(), Error> {
        self.iter().try_for_each(|element| element.read(out, at))
    }

    fn write(&mut self, values: &Values<'_>, at: &mut usize) -> Result<(), Error> {
        self.iter_mut()
            .try_for_each(|element| element.write(values, at))
    }
}

impl<E: Enumeration> Value for E {}
impl<E: Enumeration> FmiType for E {
    type DefaultVariability = Discrete;
    const KIND: Kind = Kind::Enumeration;
    const DIMS: Dims = Dims::SCALAR;
    const ENUMERATION: Option<&'static EnumerationType> = Some(&EnumerationType {
        name: E::NAME,
        items: E::ITEMS,
    });

    fn read(&self, out: &mut ValuesMut<'_>, at: &mut usize) -> Result<(), Error> {
        match out {
            ValuesMut::Int64(out) => put(out, at, self.to_i64()),
            other => Err(mismatch(Self::KIND, other.type_name())),
        }
    }

    fn write(&mut self, values: &Values<'_>, at: &mut usize) -> Result<(), Error> {
        match values {
            Values::Int64(values) => {
                let value = take(values, at)?;
                *self = E::from_i64(value)
                    .ok_or_else(|| Error::new(format!("{value} is not an item of {}", E::NAME)))?;
                Ok(())
            }
            other => Err(mismatch(Self::KIND, other.type_name())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrays_have_their_dimensions_outermost_first() {
        assert_eq!(<f64 as FmiType>::DIMS.as_slice(), &[] as &[usize]);
        assert_eq!(<[[f32; 3]; 2] as FmiType>::DIMS.as_slice(), &[2, 3]);
        assert_eq!(<[[f32; 3]; 2] as FmiType>::DIMS.count(), 6);
        assert_eq!(<[[f32; 3]; 2] as FmiType>::KIND, Kind::Float32);
    }

    #[test]
    fn an_array_reads_and_writes_row_major() {
        let mut matrix = [[1.0, 2.0], [3.0, 4.0]];
        let mut out = [0.0; 4];
        let mut at = 0;
        matrix
            .read(&mut ValuesMut::Float64(&mut out), &mut at)
            .unwrap();
        assert_eq!((out, at), ([1.0, 2.0, 3.0, 4.0], 4));

        let mut at = 0;
        matrix
            .write(&Values::Float64(&[5.0, 6.0, 7.0, 8.0]), &mut at)
            .unwrap();
        assert_eq!(matrix, [[5.0, 6.0], [7.0, 8.0]]);
    }

    #[test]
    fn an_enumeration_travels_as_int64_and_refuses_a_value_with_no_item() {
        use crate::test_model::Sign;
        let mut sign = Sign::Zero;
        let mut at = 0;
        sign.write(&Values::Int64(&[2]), &mut at).unwrap();
        assert_eq!(sign, Sign::Positive);

        let mut at = 0;
        let error = sign.write(&Values::Int64(&[7]), &mut at).unwrap_err();
        assert_eq!(error.message(), "7 is not an item of Sign");
        assert_eq!(sign, Sign::Positive);
    }

    #[test]
    fn a_mismatched_type_or_a_short_buffer_is_an_error() {
        let mut x = 1.0_f64;
        let mut at = 0;
        let error = x.write(&Values::Int32(&[1]), &mut at).unwrap_err();
        assert_eq!(
            error.message(),
            "a Float64 variable cannot take Int32 values"
        );

        let mut at = 0;
        let error = [1_u8, 2].read(&mut ValuesMut::UInt8(&mut [0]), &mut at);
        assert_eq!(
            error.unwrap_err().message(),
            "too few values for the variable"
        );
    }
}
