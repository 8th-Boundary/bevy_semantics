//! Stable semantic identities for Rust types.

use crate::Kind;

/// A Rust type with an explicit, stable semantic identity.
///
/// `KIND_NAME` is the canonical semantic name and `KIND` must equal
/// `kind!(KIND_NAME)`. Unlike [`crate::SemanticComponent`], this trait does not
/// require the type to be stored as a Bevy component.
///
/// Implementations should normally be generated with
/// `#[derive(SemanticType)]` or [`crate::semantic_type!`].
pub trait SemanticType: Send + Sync + 'static {
    const KIND: Kind;
    const KIND_NAME: &'static str;
}
