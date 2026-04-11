//! Public scalar edge weight type.

use bevy_reflect::Reflect;

/// Public scalar edge weight.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
pub struct Weight(i64);

impl Weight {
    /// Return the raw `i64` value.
    #[inline]
    pub const fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for Weight {
    #[inline]
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<Weight> for i64 {
    #[inline]
    fn from(value: Weight) -> Self {
        value.0
    }
}

impl std::fmt::Display for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
