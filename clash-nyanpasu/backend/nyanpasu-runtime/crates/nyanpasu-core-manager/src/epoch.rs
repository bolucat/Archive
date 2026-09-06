//! The manager's epoch identifier.

use std::{
    fmt,
    num::{NonZeroU64, TryFromIntError},
};

/// One epoch issued by the manager's allocator.
///
/// Never zero: allocation starts at 1 and "no epoch" is spelled
/// `Option<Epoch>`, never a zero sentinel. Deliberately not convertible from a
/// bare `u64` — a number decoded from an artifact filename, a pid record, or
/// the wire is not an epoch until something vouches for it. Cross that
/// boundary with [`Epoch::try_from`] inbound and [`Epoch::get`] outbound.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(NonZeroU64);

impl Epoch {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for Epoch {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::try_from(value).map(Self)
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
pub(crate) const fn epoch(value: u64) -> Epoch {
    match Epoch::new(value) {
        Some(epoch) => epoch,
        None => panic!("a test epoch must be nonzero"),
    }
}
