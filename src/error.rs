//! `Error`: what a refused call reports.

use std::borrow::Cow;
use std::fmt;

/// A refused call. The message goes to the importer's logger, and the call returns
/// `fmi3Error`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(Cow<'static, str>);

impl Error {
    #[must_use]
    pub fn new(message: impl Into<Cow<'static, str>>) -> Self {
        Self(message.into())
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
