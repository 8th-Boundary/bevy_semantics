# Changelog

All notable changes to `bevy_semantics` will be documented in this file.

## 0.2.0 - Unreleased

- Updated Bevy compatibility to `0.19.x`.
- Removed the unintended `Resource` derive from `SemanticEdge` to keep semantic edge records out of Bevy 0.19's resources-as-components model.
- Added release metadata for crates.io/docs.rs publication.
- Added compile-time `kind!`, `SemanticComponent`, its derive, and explicit concrete generic-instantiation support.
- Added world-local, bidirectional `Kind`/`ComponentId` registration through `SemanticWorldExt` and `SemanticAppExt`.
- Added identity pinning and explicit tombstone/revival for static component kinds.

## 0.1.0

- Initial semantic registry, snapshot, query, Bevy plugin, command queue, examples, tests, and benchmarks.
