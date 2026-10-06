//! Clocks. A periodic clock's schedule is a marker type that implements [`Periodic`].
//! [`Clock<P>`] declares the clock variable, and a variable whose clock parameter is `P`
//! ticks with it.

use core::fmt;
use core::marker::PhantomData;
use core::time::Duration;

use super::Discrete;
use crate::{Error, Values, ValuesMut};

/// A periodic clock's schedule. Implement it on a marker type, declare the clock with
/// a `Clock<Marker>` field, and give each variable that ticks with it `Marker` as its
/// clock parameter.
///
/// ```
/// use core::time::Duration;
///
/// pub struct Fast;
///
/// impl fmite::Periodic for Fast {
///     const INTERVAL: Duration = Duration::from_millis(10);
///     const PRIORITY: u32 = 0;
/// }
/// ```
pub trait Periodic: 'static {
    /// The time between two ticks.
    const INTERVAL: Duration;
    /// The clock's `priority`. Under Scheduled Execution, the importer runs the
    /// partition of a lower number first when two are due at once.
    const PRIORITY: u32;
}

/// A clock's interval and priority: what the model description says about it, and how
/// fmite tells two clocks apart. Two clocks with the same schedule would be the same
/// partition, so [`check`](crate::check) refuses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub interval: Duration,
    pub priority: u32,
}

impl Schedule {
    #[must_use]
    pub const fn of<P: Periodic>() -> Self {
        Self {
            interval: P::INTERVAL,
            priority: P::PRIORITY,
        }
    }

    /// `==`, in `const`.
    #[must_use]
    pub const fn same(self, other: Self) -> bool {
        self.interval.as_nanos() == other.interval.as_nanos() && self.priority == other.priority
    }

    /// Whether this is `P`'s schedule.
    #[must_use]
    pub const fn is<P: Periodic>(self) -> bool {
        self.same(Self::of::<P>())
    }
}

/// The clock parameter of a variable with variability `V`: `()` for an unclocked
/// variable, or the [`Periodic`] marker of the clock it ticks with.
#[diagnostic::on_unimplemented(
    message = "a variable that ticks with clock `{Self}` cannot be `{V}`",
    note = "a clocked variable is discrete"
)]
pub trait Clocking<V> {
    const SCHEDULE: Option<Schedule>;
}

impl<V> Clocking<V> for () {
    const SCHEDULE: Option<Schedule> = None;
}

impl<P: Periodic> Clocking<Discrete> for P {
    const SCHEDULE: Option<Schedule> = Some(Schedule::of::<P>());
}

/// A periodic input clock. It has no value the model reads. The importer ticks it,
/// and fmite tells the model which clock ticked.
pub struct Clock<P: Periodic>(PhantomData<P>);

impl<P: Periodic> Clock<P> {
    /// A clock has no value an `fmi3Get{Type}` reads. The instance refuses the call
    /// first, so this is only here for a `Variables::get` that calls every field.
    ///
    /// # Errors
    ///
    /// Always.
    pub fn get_into(&self, _: ValuesMut<'_>) -> Result<(), Error> {
        Err(Error::new("a clock has no value to get"))
    }

    /// As [`get_into`](Self::get_into).
    ///
    /// # Errors
    ///
    /// Always.
    #[doc(hidden)]
    pub fn importer_set(&mut self, _: Values<'_>) -> Result<(), Error> {
        Err(Error::new("a clock has no value to set"))
    }
}

impl<P: Periodic> Default for Clock<P> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<P: Periodic> Clone for Clock<P> {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl<P: Periodic> fmt::Debug for Clock<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Clock({:?})", Schedule::of::<P>())
    }
}
