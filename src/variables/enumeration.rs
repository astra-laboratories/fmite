//! Enumerations. A Rust enum is an FMI `Enumeration`, and its items become the
//! model description's `<EnumerationType>`.
//!
//! ```
//! #[derive(Clone, Copy, Default)]
//! #[repr(i64)]
//! enum Mode {
//!     #[default]
//!     Off = 1,
//!     Heat = 2,
//! }
//!
//! impl fmite::Enumeration for Mode {
//!     const NAME: &'static str = "Mode";
//!     const ITEMS: &'static [(&'static str, i64)] = &[("Off", 1), ("Heat", 2)];
//!
//!     fn to_i64(self) -> i64 {
//!         self as i64
//!     }
//!
//!     fn from_i64(value: i64) -> Option<Self> {
//!         match value {
//!             1 => Some(Self::Off),
//!             2 => Some(Self::Heat),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! let _: fmite::Output<Mode>;
//! ```
//!
//! An enumeration is discrete and has no unit. The crate docs test both.

/// A Rust enum that is an FMI enumeration. `ITEMS` lists each item's name and value.
/// If the importer sends a value `from_i64` rejects, it gets `fmi3Error`, so the model
/// only ever holds a valid item.
pub trait Enumeration: Copy + 'static {
    const NAME: &'static str;
    const ITEMS: &'static [(&'static str, i64)];

    fn to_i64(self) -> i64;
    fn from_i64(value: i64) -> Option<Self>;
}

/// An `<EnumerationType>`: its name and its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnumerationType {
    pub name: &'static str,
    pub items: &'static [(&'static str, i64)],
}
