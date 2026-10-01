//! `Field`, the value a model variable holds, and its five aliases.

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use super::{
    CausalityMarker, Fixed, FmiType, InitialFor, UnitOf, VariabilityFor, VariabilityOf, Writable,
    causality,
};

/// Every bound a [`Field`] needs, in one place.
pub trait Valid {}

impl<C, T, U, V, I> Valid for (C, T, U, V, I)
where
    C: CausalityMarker,
    T: FmiType,
    U: UnitOf<T>,
    V: VariabilityOf<T>,
    I: InitialFor<C, V>,
{
}

/// A model variable holding a `T`. Use it through the aliases: [`Input`],
/// [`Parameter`], [`CalculatedParameter`], [`Output`], [`Local`].
#[repr(transparent)]
pub struct Field<C, T, U, V, I>(T, PhantomData<(C, U, V, I)>)
where
    (C, T, U, V, I): Valid;

pub type Parameter<T, U = (), V = Fixed> =
    Field<causality::Parameter, T, U, V, <V as VariabilityFor<causality::Parameter>>::Initial>;

pub type CalculatedParameter<T, U = (), V = Fixed> = Field<
    causality::CalculatedParameter,
    T,
    U,
    V,
    <V as VariabilityFor<causality::CalculatedParameter>>::Initial,
>;

pub type Input<T, U = (), V = <T as FmiType>::Variability> =
    Field<causality::Input, T, U, V, <V as VariabilityFor<causality::Input>>::Initial>;

pub type Output<
    T,
    U = (),
    V = <T as FmiType>::Variability,
    I = <V as VariabilityFor<causality::Output>>::Initial,
> = Field<causality::Output, T, U, V, I>;

pub type Local<
    T,
    U = (),
    V = <T as FmiType>::Variability,
    I = <V as VariabilityFor<causality::Local>>::Initial,
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
}
