//! `Values` and `ValuesMut`: the typed slices a get or set carries, one variant per C
//! function. An enumeration travels as `Int64`, as it does across the C API.

use crate::Error;

macro_rules! values {
    ($($variant:ident($ty:ty),)*) => {
        /// The values of one set call for one variable, in the variable's FMI type.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum Values<'a> {
            $($variant(&'a [$ty]),)*
        }

        /// The buffer of one get call for one variable, in the variable's FMI type.
        #[derive(Debug, PartialEq)]
        pub enum ValuesMut<'a> {
            $($variant(&'a mut [$ty]),)*
        }

        impl<'a> Values<'a> {
            /// The first `n` values, which this then no longer holds; `None` if there
            /// are fewer.
            pub fn split_front(&mut self, n: usize) -> Option<Values<'a>> {
                match self {
                    $(Self::$variant(values) => {
                        let (head, tail) = values.split_at_checked(n)?;
                        *values = tail;
                        Some(Self::$variant(head))
                    })*
                }
            }

            #[must_use]
            pub fn len(&self) -> usize {
                match self {
                    $(Self::$variant(values) => values.len(),)*
                }
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }

            /// The element name of the variable type these values belong to.
            #[must_use]
            pub fn type_name(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => stringify!($variant),)*
                }
            }
        }

        impl<'a> ValuesMut<'a> {
            /// The first `n` slots, which this then no longer holds; `None` if there
            /// are fewer.
            pub fn split_front(&mut self, n: usize) -> Option<ValuesMut<'a>> {
                match self {
                    $(Self::$variant(values) => {
                        if n > values.len() {
                            return None;
                        }
                        let (head, tail) = core::mem::take(values).split_at_mut(n);
                        *values = tail;
                        Some(Self::$variant(head))
                    })*
                }
            }

            #[must_use]
            pub fn len(&self) -> usize {
                match self {
                    $(Self::$variant(values) => values.len(),)*
                }
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }

            /// The element name of the variable type this buffer belongs to.
            #[must_use]
            pub fn type_name(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => stringify!($variant),)*
                }
            }
        }

        $(
            impl Carrier for $ty {
                fn values(values: &[Self]) -> Values<'_> {
                    Values::$variant(values)
                }

                fn values_mut(values: &mut [Self]) -> ValuesMut<'_> {
                    ValuesMut::$variant(values)
                }
            }
        )*
    };
}

/// A scalar type a get or set function carries: `f64` for `fmi3GetFloat64`.
pub trait Carrier: Copy + 'static {
    fn values(values: &[Self]) -> Values<'_>;
    fn values_mut(values: &mut [Self]) -> ValuesMut<'_>;
}

values! {
    Float32(f32),
    Float64(f64),
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Int64(i64),
    UInt64(u64),
    Boolean(bool),
}

/// The error for `left` values a call carried beyond what its variables take.
///
/// # Errors
///
/// When `left` is not zero.
pub fn surplus(left: usize) -> Result<(), Error> {
    if left == 0 {
        Ok(())
    } else {
        Err(Error::new(format!(
            "{left} values more than the variables take"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_front_takes_a_prefix_or_nothing() {
        let mut values = Values::Int32(&[1, 2, 3]);
        assert_eq!(values.split_front(2), Some(Values::Int32(&[1, 2])));
        assert_eq!(values.split_front(2), None);
        assert_eq!(values, Values::Int32(&[3]));

        let mut buffer = [0_u8; 3];
        let mut out = ValuesMut::UInt8(&mut buffer);
        if let Some(ValuesMut::UInt8(head)) = out.split_front(1) {
            head[0] = 7;
        }
        assert_eq!(out.len(), 2);
        assert_eq!(buffer, [7, 0, 0]);
    }
}
