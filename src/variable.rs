//! Model variables. Causality, variability, initial and unit are type parameters, and
//! FMI 3.0 Table 22 is encoded once, as the impls of [`VariabilityFor`] and
//! [`InitialFor`].
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
//!     const BASE: fmite::BaseUnit = fmite::BaseUnit::METRE;
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

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

mod sealed {
    pub trait Sealed {}
}
use sealed::Sealed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causality {
    Parameter,
    CalculatedParameter,
    Input,
    Output,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variability {
    Constant,
    Fixed,
    Tunable,
    Discrete,
    Continuous,
}

/// `approx` is left out until Model Exchange needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Initial {
    Exact,
    Calculated,
}

/// Causality markers, in a module because `Input` and `Output` also name the wrappers.
pub mod causality {
    pub struct Parameter;
    pub struct CalculatedParameter;
    pub struct Input;
    pub struct Output;
    pub struct Local;
}

pub struct Constant;
pub struct Fixed;
pub struct Tunable;
pub struct Discrete;
pub struct Continuous;

pub struct Exact;
pub struct Calculated;

pub trait CausalityMarker: Sealed {
    const CAUSALITY: Causality;
}

pub trait VariabilityMarker: Sealed {
    const VARIABILITY: Variability;
}

pub trait InitialMarker: Sealed {
    const INITIAL: Initial;
}

macro_rules! markers {
    ($trait:ident::$name:ident: $value:ident { $($ty:ty => $variant:ident,)* }) => {
        $(
            impl Sealed for $ty {}
            impl $trait for $ty {
                const $name: $value = $value::$variant;
            }
        )*
    };
}

markers!(CausalityMarker::CAUSALITY: Causality {
    causality::Parameter => Parameter,
    causality::CalculatedParameter => CalculatedParameter,
    causality::Input => Input,
    causality::Output => Output,
    causality::Local => Local,
});

markers!(VariabilityMarker::VARIABILITY: Variability {
    Constant => Constant,
    Fixed => Fixed,
    Tunable => Tunable,
    Discrete => Discrete,
    Continuous => Continuous,
});

markers!(InitialMarker::INITIAL: Initial {
    Exact => Exact,
    Calculated => Calculated,
});

/// A row of Table 22: `Self` is a legal variability for causality `C`. `Initial` is the
/// default.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable cannot have variability `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait VariabilityFor<C>: VariabilityMarker {
    type Initial: InitialFor<C, Self>;
}

/// A cell of Table 22: `Self` is a legal initial for causality `C` with variability `V`.
#[diagnostic::on_unimplemented(
    message = "a `{C}` variable with variability `{V}` cannot have initial `{Self}`",
    note = "FMI 3.0 Table 22"
)]
pub trait InitialFor<C, V: ?Sized>: InitialMarker {}

macro_rules! rows {
    ($($c:ident: $v:ident => $i:ident,)*) => {
        $(impl VariabilityFor<causality::$c> for $v { type Initial = $i; })*
    };
}

macro_rules! cells {
    ($($c:ident: $v:ident => $i:ident,)*) => {
        $(impl InitialFor<causality::$c, $v> for $i {})*
    };
}

rows! {
    Parameter: Fixed => Exact,
    Parameter: Tunable => Exact,
    CalculatedParameter: Fixed => Calculated,
    CalculatedParameter: Tunable => Calculated,
    Input: Discrete => Exact,
    Input: Continuous => Exact,
    Output: Constant => Exact,
    Output: Discrete => Calculated,
    Output: Continuous => Calculated,
    Local: Constant => Exact,
    Local: Fixed => Calculated,
    Local: Tunable => Calculated,
    Local: Discrete => Calculated,
    Local: Continuous => Calculated,
}

cells! {
    Parameter: Fixed => Exact,
    Parameter: Tunable => Exact,
    CalculatedParameter: Fixed => Calculated,
    CalculatedParameter: Tunable => Calculated,
    Input: Discrete => Exact,
    Input: Continuous => Exact,
    Output: Constant => Exact,
    Output: Discrete => Exact,
    Output: Discrete => Calculated,
    Output: Continuous => Exact,
    Output: Continuous => Calculated,
    Local: Constant => Exact,
    Local: Fixed => Calculated,
    Local: Tunable => Calculated,
    Local: Discrete => Exact,
    Local: Discrete => Calculated,
    Local: Continuous => Exact,
    Local: Continuous => Calculated,
}

/// A Rust type that is an FMI type. `Variability` is the standard's default for it.
pub trait FmiType: Sealed {
    type Variability: VariabilityMarker;
}

/// `f32` and `f64`: the only types that can be continuous or carry a unit.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a float",
    note = "only floats are continuous or carry a unit"
)]
pub trait Float: FmiType {}

macro_rules! fmi_types {
    ($variability:ident: $($ty:ty),*) => {
        $(
            impl Sealed for $ty {}
            impl FmiType for $ty {
                type Variability = $variability;
            }
        )*
    };
}

fmi_types!(Continuous: f32, f64);
fmi_types!(Discrete: i8, i16, i32, i64, u8, u16, u32, u64, bool);

impl Float for f32 {}
impl Float for f64 {}

impl<T: FmiType, const N: usize> Sealed for [T; N] {}
impl<T: FmiType, const N: usize> FmiType for [T; N] {
    type Variability = T::Variability;
}

/// `Self` is a legal variability for a variable of type `T`.
#[diagnostic::on_unimplemented(
    message = "a `{T}` variable cannot be `{Self}`",
    note = "only floats are continuous"
)]
pub trait VariabilityOf<T> {}

impl<T: FmiType> VariabilityOf<T> for Constant {}
impl<T: FmiType> VariabilityOf<T> for Fixed {}
impl<T: FmiType> VariabilityOf<T> for Tunable {}
impl<T: FmiType> VariabilityOf<T> for Discrete {}
impl<T: Float> VariabilityOf<T> for Continuous {}
impl<T, const N: usize> VariabilityOf<[T; N]> for Continuous where Self: VariabilityOf<T> {}

/// A unit as FMI defines it: exponents of the SI base units plus `rad`, and the map
/// `factor * value + offset` into them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BaseUnit {
    pub kg: i8,
    pub m: i8,
    pub s: i8,
    pub a: i8,
    pub k: i8,
    pub mol: i8,
    pub cd: i8,
    pub rad: i8,
    pub factor: f64,
    pub offset: f64,
}

impl BaseUnit {
    pub const ONE: Self = Self {
        kg: 0,
        m: 0,
        s: 0,
        a: 0,
        k: 0,
        mol: 0,
        cd: 0,
        rad: 0,
        factor: 1.0,
        offset: 0.0,
    };
    pub const KILOGRAM: Self = Self { kg: 1, ..Self::ONE };
    pub const METRE: Self = Self { m: 1, ..Self::ONE };
    pub const SECOND: Self = Self { s: 1, ..Self::ONE };
    pub const AMPERE: Self = Self { a: 1, ..Self::ONE };
    pub const KELVIN: Self = Self { k: 1, ..Self::ONE };
    pub const MOLE: Self = Self {
        mol: 1,
        ..Self::ONE
    };
    pub const CANDELA: Self = Self { cd: 1, ..Self::ONE };
    pub const RADIAN: Self = Self {
        rad: 1,
        ..Self::ONE
    };

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

    const fn combine(self, other: Self, sign: i8, factor: f64) -> Self {
        Self {
            kg: self.kg + sign * other.kg,
            m: self.m + sign * other.m,
            s: self.s + sign * other.s,
            a: self.a + sign * other.a,
            k: self.k + sign * other.k,
            mol: self.mol + sign * other.mol,
            cd: self.cd + sign * other.cd,
            rad: self.rad + sign * other.rad,
            factor,
            offset: 0.0,
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
#[diagnostic::on_unimplemented(
    message = "a `{T}` variable cannot have unit `{Self}`",
    note = "only floats have a unit"
)]
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

/// `C` with variability `V` is computed by the model, at every step.
pub trait Writable<V> {}

impl Writable<Discrete> for causality::Output {}
impl Writable<Continuous> for causality::Output {}
impl Writable<Discrete> for causality::Local {}
impl Writable<Continuous> for causality::Local {}

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

    struct Celsius;

    impl Unit for Celsius {
        const NAME: &'static str = "degC";
        const BASE: BaseUnit = BaseUnit::KELVIN.offset(273.15);
    }

    struct KmPerHour;

    impl Unit for KmPerHour {
        const NAME: &'static str = "km/h";
        const BASE: BaseUnit = BaseUnit::METRE
            .scaled(1000.0)
            .per(BaseUnit::SECOND.scaled(3600.0));
    }

    fn describe<F: Describe>() -> (Causality, Variability, Initial) {
        let v = Variable::new::<F>("x", 0);
        (v.causality, v.variability, v.initial)
    }

    #[test]
    fn defaults_follow_the_value_type() {
        use Causality as C;
        use Initial as I;
        use Variability as V;

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
        use Causality as C;
        use Initial as I;
        use Variability as V;

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
        use Causality as C;
        use Initial as I;
        use Variability as V;

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
            base: BaseUnit {
                k: 1,
                offset: 273.15,
                ..BaseUnit::ONE
            },
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
    fn derived_units_compose() {
        let speed = KmPerHour::BASE;
        assert_eq!((speed.m, speed.s), (1, -1));
        assert!((speed.factor - 1000.0 / 3600.0).abs() < 1e-12);

        let newton = BaseUnit::KILOGRAM
            .times(BaseUnit::METRE)
            .per(BaseUnit::SECOND.times(BaseUnit::SECOND));
        assert_eq!((newton.kg, newton.m, newton.s), (1, 1, -2));
    }

    #[test]
    fn composing_drops_the_offset() {
        let rate = Celsius::BASE.per(BaseUnit::SECOND);
        assert_eq!(rate.offset, 0.0);
        assert_eq!((rate.k, rate.s), (1, -1));
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

    #[test]
    fn a_field_reads_through_and_writes_when_computed() {
        let input = Input::<f64>::new(2.0);
        let mut output = Output::<f64>::default();
        *output = *input * 3.0;
        assert_eq!(*output, 6.0);
        assert_eq!(*output.clone(), 6.0);
    }
}
