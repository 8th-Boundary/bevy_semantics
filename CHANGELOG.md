# Changelog

All notable changes to `bevy_semantics` will be documented in this file.

## 0.2.0 - Unreleased

- Updated Bevy compatibility to `0.19.x`.
- Removed the unintended `Resource` derive from `SemanticEdge` to keep semantic edge records out of Bevy 0.19's resources-as-components model.
- Added release metadata for crates.io/docs.rs publication.

## 0.1.0

- Initial semantic registry, snapshot, query, Bevy plugin, command queue, examples, tests, and benchmarks.
