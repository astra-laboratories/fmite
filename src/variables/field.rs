//! `Field`, the value a model variable holds, and its five aliases.

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use super::causality::{self, Causality};
use super::unit::UnitOf;
use super::{Fixed, FmiType, InitialFor, VariabilityFor, VariabilityOf, Writable};
use crate::values::surplus;
use crate::{Error, Values, ValuesMut};

/// Every bound a [`Field`] needs, in one place.
pub trait Valid {}

impl<C, T, U, V, I> Valid for (C, T, U, V, I)
where
    C: Causality,
    T: FmiType,
    U: UnitOf<T>,
    V: VariabilityFor<C> + VariabilityOf<T>,
    I: InitialFor<C, V>,
{
}

/// A model variable holding a `T`. Use it through the aliases: [`Input`],
/// [`Parameter`], [`CalculatedParameter`], [`Output`], [`Local`].
#[repr(transparent)]
pub struct Field<C, T, U, V, I>(T, PhantomData<(C, U, V, I)>)
where
    (C, T, U, V, I): Valid;

pub type Parameter<T, U = (), V = Fixed> = Field<
    causality::Parameter,
    T,
    U,
    V,
    <V as VariabilityFor<causality::Parameter>>::DefaultInitial,
>;

pub type CalculatedParameter<T, U = (), V = Fixed> = Field<
    causality::CalculatedParameter,
    T,
    U,
    V,
    <V as VariabilityFor<causality::CalculatedParameter>>::DefaultInitial,
>;

pub type Input<T, U = (), V = <T as FmiType>::DefaultVariability> =
    Field<causality::Input, T, U, V, <V as VariabilityFor<causality::Input>>::DefaultInitial>;

pub type Output<
    T,
    U = (),
    V = <T as FmiType>::DefaultVariability,
    I = <V as VariabilityFor<causality::Output>>::DefaultInitial,
> = Field<causality::Output, T, U, V, I>;

pub type Local<
    T,
    U = (),
    V = <T as FmiType>::DefaultVariability,
    I = <V as VariabilityFor<causality::Local>>::DefaultInitial,
> = Field<causality::Local, T, U, V, I>;

impl<C, T, U, V, I> Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value, PhantomData)
    }
}

impl<C, T, U, V, I> Deref for Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<C: Writable<V>, T, U, V, I> DerefMut for Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<C, T: FmiType, U, V, I> Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    /// Writes the value to a get buffer, for a hand-written `Variables::get`.
    ///
    /// # Errors
    ///
    /// When `out` is not of this field's FMI type, or not of its length.
    pub fn get_into(&self, mut out: ValuesMut<'_>) -> Result<(), Error> {
        let mut at = 0;
        self.0.read(&mut out, &mut at)?;
        surplus(out.len() - at)
    }

    /// Sets the value from the importer's values, for `Variables::set`. Model code has
    /// no use for it, and the types cannot stop it calling it; they stop it writing an
    /// input by accident, not on purpose. Atomic: on an error the value is unchanged.
    ///
    /// # Errors
    ///
    /// When `values` are not of this field's FMI type, or not of its length, or an
    /// enumeration value has no item.
    #[doc(hidden)]
    pub fn importer_set(&mut self, values: Values<'_>) -> Result<(), Error> {
        let mut next = self.0;
        let mut at = 0;
        next.write(&values, &mut at)?;
        surplus(values.len() - at)?;
        self.0 = next;
        Ok(())
    }
}

impl<C, T: Default, U, V, I> Default for Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<C, T: Clone, U, V, I> Clone for Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    fn clone(&self) -> Self {
        Self::new(self.0.clone())
    }
}

impl<C, T: fmt::Debug, U, V, I> fmt::Debug for Field<C, T, U, V, I>
where
    (C, T, U, V, I): Valid,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_reads_through_and_writes_when_computed() {
        let input = Input::<f64>::new(2.0);
        let mut output = Output::<f64>::default();
        *output = *input * 3.0;
        assert_eq!(*output, 6.0);
        assert_eq!(*output.clone(), 6.0);
    }

    #[test]
    fn the_importer_sets_all_or_nothing() {
        let mut cells = Input::<[f64; 2]>::default();
        cells.importer_set(Values::Float64(&[1.0, 2.0])).unwrap();
        assert!(
            cells
                .importer_set(Values::Float64(&[3.0, 4.0, 5.0]))
                .is_err()
        );
        assert!(cells.importer_set(Values::Float64(&[3.0])).is_err());
        assert_eq!(*cells, [1.0, 2.0]);

        let mut out = [0.0; 2];
        cells.get_into(ValuesMut::Float64(&mut out)).unwrap();
        assert_eq!(out, [1.0, 2.0]);
    }
}
