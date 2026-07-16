//! Error types returned by `bevy_semantics` public APIs.

use crate::kind::Kind;

/// Errors emitted by semantic authoring and query operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticError {
    InvalidName {
        name: String,
    },
    KindHashCollision {
        kind: Kind,
        existing_name: String,
        requested_name: String,
    },
    UnknownKind {
        kind: Kind,
    },
    CannotUnregisterCoreKind {
        kind: Kind,
    },
    CannotUnregisterStaticComponentKind {
        kind: Kind,
        name: String,
    },
    TombstonedKind {
        kind: Kind,
    },
    CannotUseReservedNamespace {
        namespace: String,
    },
    KindTypeConflict {
        kind: Kind,
        existing_type_name: String,
        requested_type_name: String,
    },
    StaticKindMismatch {
        type_name: &'static str,
        name: &'static str,
        declared: Kind,
        generated: Kind,
    },
    KindRegistrationMismatch {
        name: &'static str,
        declared: Kind,
        generated: Kind,
    },
    KindComponentConflict {
        kind: Kind,
        existing_component: usize,
        requested_component: usize,
    },
    ComponentKindConflict {
        component: usize,
        existing_kind: Kind,
        requested_kind: Kind,
    },
    EmptySeedSet,
    InvalidQueryState {
        query: &'static str,
        reason: String,
    },
    StaleCompiledQuery {
        domain: &'static str,
        compiled_version: u64,
        current_version: u64,
    },
    MalformedCommand {
        command: &'static str,
        reason: String,
    },
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticError::InvalidName { name } => {
                write!(f, "invalid canonical name '{name}'")
            }
            SemanticError::KindHashCollision {
                kind,
                existing_name,
                requested_name,
            } => write!(
                f,
                "kind hash collision for {kind}: existing '{existing_name}', requested '{requested_name}'"
            ),
            SemanticError::UnknownKind { kind } => write!(f, "unknown kind '{kind}'"),
            SemanticError::CannotUnregisterCoreKind { kind } => {
                write!(f, "cannot unregister core kind '{kind}'")
            }
            SemanticError::CannotUnregisterStaticComponentKind { kind, name } => write!(
                f,
                "cannot unregister static component kind '{name}' ({kind}); tombstone it instead"
            ),
            SemanticError::TombstonedKind { kind } => {
                write!(f, "kind '{kind}' is tombstoned")
            }
            SemanticError::CannotUseReservedNamespace { namespace } => {
                write!(f, "cannot use reserved namespace '{namespace}'")
            }
            SemanticError::KindTypeConflict {
                kind,
                existing_type_name,
                requested_type_name,
            } => write!(
                f,
                "kind '{kind}' already bound to type '{existing_type_name}', cannot bind to '{requested_type_name}'"
            ),
            SemanticError::StaticKindMismatch {
                type_name,
                name,
                declared,
                generated,
            } => write!(
                f,
                "static semantic kind mismatch for {type_name}: name '{name}' generates {generated}, but the type declares {declared}"
            ),
            SemanticError::KindRegistrationMismatch {
                name,
                declared,
                generated,
            } => write!(
                f,
                "kind registration mismatch: name '{name}' generates {generated}, but the registration contains {declared}"
            ),
            SemanticError::KindComponentConflict {
                kind,
                existing_component,
                requested_component,
            } => write!(
                f,
                "kind '{kind}' is already bound to component ID {existing_component}, cannot bind component ID {requested_component}"
            ),
            SemanticError::ComponentKindConflict {
                component,
                existing_kind,
                requested_kind,
            } => write!(
                f,
                "component ID {component} is already bound to kind '{existing_kind}', cannot bind kind '{requested_kind}'"
            ),
            SemanticError::EmptySeedSet => {
                write!(f, "traversal query requires at least one seed kind")
            }
            SemanticError::InvalidQueryState { query, reason } => {
                write!(f, "invalid {query} query state: {reason}")
            }
            SemanticError::StaleCompiledQuery {
                domain,
                compiled_version,
                current_version,
            } => write!(
                f,
                "stale compiled {domain} query: compiled at version {compiled_version}, current version is {current_version}"
            ),
            SemanticError::MalformedCommand { command, reason } => {
                write!(f, "malformed {command} command: {reason}")
            }
        }
    }
}

impl std::error::Error for SemanticError {}
