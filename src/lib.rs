//! Semantic identity, relation graphs, immutable snapshots, and cached queries for Bevy.
//!
//! The crate centers on [`SemanticRegistry`] for mutation and [`SemanticSnapshot`] for
//! cloned concurrent reads. Bevy integration is built in through [`Semantics`],
//! which forwards the common read helpers, plus [`SemanticPlaybackQueue`] and
//! [`SemanticsPlugin`], along with the command extension traits re-exported in [`prelude`].

pub mod direction;
pub mod error;
pub mod kind;
pub mod query;
pub mod registry;
pub mod snapshot;
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
    pub use crate::snapshot::SemanticSnapshot;
    pub use crate::weight::Weight;
}

pub use direction::EdgeDirection;
pub use error::SemanticError;
pub use kind::Kind;
pub use query::{
    CompiledEdgeQuery, CompiledTraversalQuery, EdgeQuery, EdgeQueryBuilder, SemanticCommand,
    SemanticEdge, TraversalQuery, TraversalQueryBuilder,
};
pub use registry::{Core, SemanticRegistry, SemanticRegistryBatch};
pub use snapshot::SemanticSnapshot;
pub use weight::Weight;

pub use plugin::{CommandsExt, SemanticEdit, SemanticPlaybackQueue, Semantics, SemanticsPlugin};
