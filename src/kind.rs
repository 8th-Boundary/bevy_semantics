//! Public semantic identity type.

use bevy_reflect::Reflect;

/// Stable public semantic identifier.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct Kind(u64);

impl Kind {
    /// Return the raw `u64` value.
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for Kind {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Kind> for u64 {
    #[inline]
    fn from(value: Kind) -> Self {
        value.0
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
