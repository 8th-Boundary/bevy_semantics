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
    CannotUnregisterBuiltInKind {
        kind: Kind,
    },
    KindTypeConflict {
        kind: Kind,
        existing_type_name: String,
        requested_type_name: String,
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
            SemanticError::CannotUnregisterBuiltInKind { kind } => {
                write!(f, "cannot unregister built-in kind '{kind}'")
            }
            SemanticError::KindTypeConflict {
                kind,
                existing_type_name,
                requested_type_name,
            } => write!(
                f,
                "kind '{kind}' already bound to type '{existing_type_name}', cannot bind to '{requested_type_name}'"
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
