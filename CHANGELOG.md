# Changelog

All notable changes to `bevy_semantics` will be documented in this file.

## 0.3.0 - Unreleased

- Targets Bevy `0.19.x` under the crate's independent semantic-versioning policy.
- Removed the unintended `Resource` derive from `SemanticEdge` to keep semantic edge records out of Bevy 0.19's resources-as-components model.
- Added release metadata for crates.io/docs.rs publication.
- Added compile-time `kind!`, `SemanticComponent`, its derive, and explicit concrete generic-instantiation support.
- Added general `SemanticType` identity, derive and concrete generic macro for
  Rust types that are not Bevy components.
- Made `SemanticComponent` a component-specific refinement of `SemanticType`;
  its derive and macro continue to generate the complete implementation.
- Added registry, world, and app helpers for validating and prewarming static
  semantic types without registering Bevy component storage.
- Pinned registered static type identities against unregistration while
  retaining tombstone support.
- Added `semantic_kinds!`, `KindRegistration`, and transactional `register_const(s)` APIs for publishing compile-time kind names without repeated string registration.
- Added compile-time constants for every seeded kind under `bevy_semantics::core`.
- Added world-local, bidirectional `Kind`/`ComponentId` registration through `SemanticWorldExt` and `SemanticAppExt`.
- Added identity pinning and explicit tombstone/revival for static component kinds.
- Simplified semantic edges to payload-free triples and added transactional tuple-based `add_edges` registration.
- Added scalar-or-collection `KindLane` Cartesian edge generation, filtering, and transactional bulk removal.
- Isolated cloned registry snapshot caches and keyed query caches by registry lineage plus version.

## 0.1.0

- Initial semantic registry, snapshot, query, Bevy plugin, command queue, examples, tests, and benchmarks.
