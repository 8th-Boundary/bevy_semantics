//! Compile-time kind declarations that retain their canonical names.

use crate::Kind;

/// A compile-time semantic kind declaration containing both its name and ID.
///
/// Graph APIs continue to use [`Kind`] directly. A `StaticKind` exists so a
/// registry can publish the canonical name associated with a compile-time ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StaticKind {
    name: &'static str,
    kind: Kind,
}

impl StaticKind {
    /// Construct a declaration from macro-generated parts.
    #[doc(hidden)]
    pub const fn from_raw_parts(name: &'static str, kind: Kind) -> Self {
        Self { name, kind }
    }

    /// Return the declared canonical name.
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Return the compile-time semantic identity.
    pub const fn kind(self) -> Kind {
        self.kind
    }
}
