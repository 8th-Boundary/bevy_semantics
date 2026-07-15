//! Semantic identity, relation graphs, immutable snapshots, and cached queries for Bevy.
//!
//! The crate centers on [`SemanticRegistry`] for mutation and [`SemanticSnapshot`] for
//! cloned concurrent reads. Bevy integration is built in through [`Semantics`],
//! which forwards the common read helpers, plus [`SemanticPlaybackQueue`] and
//! [`SemanticsPlugin`], along with the command extension traits re-exported in [`prelude`].

extern crate self as bevy_semantics;

pub mod core;
pub mod direction;
pub mod error;
pub mod kind;
pub mod query;
pub mod registry;
pub mod semantic_component;
pub mod snapshot;
pub mod static_kind;
pub mod weight;

pub mod plugin;

pub mod prelude {
    pub use crate::direction::EdgeDirection;
    pub use crate::error::SemanticError;
    pub use crate::kind::Kind;
    pub use crate::plugin::{
        CommandsExt, SemanticEdit, SemanticPlaybackQueue, Semantics, SemanticsPlugin,
    };
    pub use crate::query::{
        CompiledEdgeQuery, CompiledTraversalQuery, EdgeQuery, EdgeQueryBuilder, SemanticCommand,
        SemanticEdge, TraversalQuery, TraversalQueryBuilder,
    };
    pub use crate::registry::{Core, SemanticRegistry, SemanticRegistryBatch};
    pub use crate::semantic_component::{SemanticAppExt, SemanticComponents, SemanticWorldExt};
    pub use crate::snapshot::SemanticSnapshot;
    pub use crate::static_kind::StaticKind;
    pub use crate::weight::Weight;
    pub use crate::{kind, semantic_component, semantic_kinds, static_kind, SemanticComponent};
}

pub use direction::EdgeDirection;
pub use error::SemanticError;
pub use kind::Kind;
pub use query::{
    CompiledEdgeQuery, CompiledTraversalQuery, EdgeQuery, EdgeQueryBuilder, SemanticCommand,
    SemanticEdge, TraversalQuery, TraversalQueryBuilder,
};
pub use registry::{Core, SemanticRegistry, SemanticRegistryBatch};
pub use semantic_component::{
    SemanticAppExt, SemanticComponent, SemanticComponents, SemanticWorldExt,
};
pub use snapshot::SemanticSnapshot;
pub use static_kind::StaticKind;
pub use weight::Weight;

pub use plugin::{CommandsExt, SemanticEdit, SemanticPlaybackQueue, Semantics, SemanticsPlugin};

#[doc(hidden)]
pub use bevy_semantics_derive::__kind_raw;
pub use bevy_semantics_derive::{semantic_component, SemanticComponent};

/// Construct a stable semantic [`Kind`] from a string literal at compile time.
#[macro_export]
macro_rules! kind {
    ($name:literal) => {
        $crate::Kind::from_raw($crate::__kind_raw!($name))
    };
}

/// Declare one compile-time kind while retaining its name for registration.
#[macro_export]
macro_rules! static_kind {
    ($name:literal) => {
        $crate::StaticKind::from_raw_parts($name, $crate::kind!($name))
    };
}

/// Declare related [`Kind`] constants and a registration descriptor slice.
///
/// The generated individual constants remain plain [`Kind`] values, while the
/// named slice can be passed to `register_consts` on [`SemanticRegistry`] or
/// [`Semantics`].
#[macro_export]
macro_rules! semantic_kinds {
    (
        $(#[$group_meta:meta])*
        $vis:vis const $group:ident = {
            $(
                $(#[$kind_meta:meta])*
                $constant:ident = $name:literal
            ),+ $(,)?
        };
    ) => {
        $(
            $(#[$kind_meta])*
            $vis const $constant: $crate::Kind = $crate::kind!($name);
        )+

        $(#[$group_meta])*
        $vis const $group: &[$crate::StaticKind] = &[
            $(
                $crate::StaticKind::from_raw_parts($name, $constant)
            ),+
        ];
    };
}
