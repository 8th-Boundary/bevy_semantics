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

pub mod plugin;

pub mod prelude {
    pub use crate::direction::EdgeDirection;
    pub use crate::error::SemanticError;
    pub use crate::kind::{Kind, KindRegistration};
    pub use crate::plugin::{
        CommandsExt, SemanticEdit, SemanticPlaybackQueue, Semantics, SemanticsPlugin,
    };
    pub use crate::query::{
        cartesian_edges, CompiledEdgeQuery, CompiledTraversalQuery, EdgeQuery, EdgeQueryBuilder,
        EdgeRegistration, KindLane, SemanticCommand, SemanticEdge, TraversalQuery,
        TraversalQueryBuilder,
    };
    pub use crate::registry::{Core, SemanticRegistry, SemanticRegistryBatch};
    pub use crate::semantic_component::{SemanticAppExt, SemanticComponents, SemanticWorldExt};
    pub use crate::snapshot::SemanticSnapshot;
    pub use crate::{kind, semantic_component, semantic_kinds, SemanticComponent};
}

pub use direction::EdgeDirection;
pub use error::SemanticError;
pub use kind::{Kind, KindRegistration};
pub use query::{
    cartesian_edges, CompiledEdgeQuery, CompiledTraversalQuery, EdgeQuery, EdgeQueryBuilder,
    EdgeRegistration, KindLane, SemanticCommand, SemanticEdge, TraversalQuery,
    TraversalQueryBuilder,
};
pub use registry::{Core, SemanticRegistry, SemanticRegistryBatch};
pub use semantic_component::{
    SemanticAppExt, SemanticComponent, SemanticComponents, SemanticWorldExt,
};
pub use snapshot::SemanticSnapshot;

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
        $vis const $group: &[$crate::KindRegistration] = &[
            $(
                ($name, $constant)
            ),+
        ];
    };
}
