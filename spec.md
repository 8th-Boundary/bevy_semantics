# bevy_semantics

Production-ready semantic identity, ontology, and query infrastructure for Bevy and Rust projects.

This crate provides:
- stable semantic identities
- optional typed kinds
- a read-optimized relation graph
- compiled semantic queries
- versioned caches
- immutable concurrent readers with a single writer
- Bevy integration in the default build

It is not:
- an ECS replacement
- a general graph database
- a text search engine
- a synonym or alias system in v1
- a weighted pathfinding engine in v1

## 1. Core Rules

- Public IDs must be stable.
- Hot paths must use dense internal indices, not public IDs.
- Reads must be lock-free or near-lock-free for the common case.
- Writes are rare, staged, deterministic, and applied by a single writer.
- Query execution must support compilation and caching.
- No silent aliasing, no implicit normalization beyond canonical name trimming.
- All fallible public APIs return `Result<_, SemanticError>`.
- `SemanticError` covers collisions, unknown kinds, type conflicts, malformed commands, invalid query states, stale compiled queries, and version mismatches.

## 2. Identity Model

### 2.1 Kind

`Kind` is the public semantic identifier.

```rust
#[repr(transparent)]
pub struct Kind(u64);
```

Rules:
- `Kind` is opaque and stable.
- It is derived from a canonical UTF-8 name using `BLAKE3` truncated to 64 bits.
- The digest input is domain-separated with a fixed prefix such as `bevy_semantics.kind.v1:`.
- The 64-bit value is the little-endian interpretation of the first 8 digest bytes.
- The digest algorithm and domain prefix are chosen once for v1 and do not change without a breaking migration.
- Registration checks for digest collisions and conflicting canonical names.
- Re-registering the same canonical name returns the existing `Kind`.

Why 64-bit:
- the internal hot path uses dense IDs, so the public ID size is not performance-critical
- 64-bit is compact enough to keep the public ID cheap to store and serialize
- collision checks make the result safe even though the ID is hash-derived
- it avoids the migration pressure of a tiny public hash space
- `BLAKE3` gives us a strong, deterministic digest without relying on a randomized hasher

### 2.2 DenseKind

`DenseKind` is the internal snapshot-local index.

```rust
#[repr(transparent)]
struct DenseKind(u32);
```

Rules:
- used for array indexing, adjacency tables, caches, and bitsets
- only valid inside a given snapshot version
- may be rebuilt when the registry changes
- is never exposed in the public API

### 2.3 Weight

`Weight` is the public edge weight type.

```rust
#[repr(transparent)]
pub struct Weight(i64);
```

Rules:
- `Weight` is opaque, stable, and cheap to copy
- it uses the same signed 64-bit storage in every layer
- it is a scalar newtype, not an enum, in v1
- `Weight::default()` is zero
- `Weight` implements `From<i64>` and `From<Weight> for i64`
- comparisons are signed numeric comparisons on the inner `i64`
- if fractional semantics are needed, encode them as fixed-point integers or separate metadata

### 2.4 EdgeDirection

`EdgeDirection` selects which side of a relation to traverse.

```rust
#[repr(u8)]
pub enum EdgeDirection {
    Outgoing,
    Incoming,
    Both,
}
```

Rules:
- `Outgoing` follows `subject -> target`
- `Incoming` follows `target -> subject`
- `Both` includes both directions
- `Both` may yield duplicate candidates before deduplication

### 2.5 Canonical Names

Canonical names are the source string for a `Kind`.

Rules:
- reject empty strings
- trim leading and trailing whitespace
- preserve case in v1
- preserve UTF-8 exactly after trimming
- no alias, synonym, or fuzzy normalization in v1

Recommended form:
- `namespace::local_name`

The crate seeds its own core kinds with plain names such as `Relation` and `is_a`.
The `Core` namespace is reserved for user code and cannot be used.

### 2.6 Typed Kinds

Typed kinds bind a Rust type to a `Kind`.

Rules:
- the binding key is `TypeId`
- default name is `std::any::type_name::<T>()`
- explicit override is allowed through a typed registration API
- typed binding must not silently change an existing binding

Examples:
- `kind_of::<Health>()`
- `typed_kind_named::<Health>("Health")`

## 3. Registry

The mutable registry owns name resolution and typed-kind binding. It maintains internal dense remapping for snapshot construction.

Required lookups:
- name -> `Kind`
- `Kind` -> name
- `TypeId` -> `Kind`
- `Kind` -> `TypeId`

Suggested metadata record:

```rust
struct KindMeta {
    kind: Kind,
    name: Arc<str>,
    type_id: Option<TypeId>,
    flags: KindFlags,
}
```

Implementation may keep separate internal dense remap tables.

Flags should at minimum distinguish core, typed, and user-defined kinds.

Rules:
- registered kinds are immutable after creation
- core kinds are seeded deterministically
- registration failures must be explicit
- the registry is the source of truth for public IDs

Direct registry API:
- `register_kind(name) -> Result<Kind, SemanticError>`
- `typed_kind::<T>() -> Result<Kind, SemanticError>`
- `typed_kind_named::<T>(name) -> Result<Kind, SemanticError>`
- `unregister_kind(kind) -> Result<bool, SemanticError>`
- `unregister_namespace(namespace) -> Result<bool, SemanticError>`
- `add_edge(subject, relation, target) -> Result<bool, SemanticError>`
- `add_edge_weighted(subject, relation, target, weight) -> Result<bool, SemanticError>`
- `remove_edge(subject, relation, target) -> Result<bool, SemanticError>`
- `has_edge(subject, relation, target) -> bool`
- `apply_commands(commands) -> Result<bool, SemanticError>`
- `batch() -> SemanticRegistryBatch`
- `core() -> Core`
- `kind(name) -> Option<Kind>`
- `kind_of::<T>() -> Option<Kind>`
- `name(kind) -> Option<&str>`
- `type_id(kind) -> Option<TypeId>`
- `snapshot() -> SemanticSnapshot`
- `snapshot_cached() -> Arc<SemanticSnapshot>`

`SemanticRegistryBatch` mirrors the mutating registry methods and exposes `snapshot()` as the terminal step for one-shot rebuilds after a bulk authoring pass.
`snapshot_cached()` returns a cached `Arc<SemanticSnapshot>` when the registry version matches the last built snapshot.

Registration rules:
- registering the same canonical name returns the existing `Kind`
- typed registration may reuse an existing canonical name only when the type binding is compatible
- conflicting name or type bindings return `SemanticError`
- unregistering a kind removes its incident edges and any type binding
- unregistering a namespace removes every kind whose canonical name starts with that namespace prefix, along with incident edges and type bindings
- namespace removal is prefix-based on canonical names
- the reserved `Core` namespace cannot be removed in any case variant
- core kinds cannot be unregistered

## 4. Core

Ship these core kinds in v1:
- `Relation`
- `Namespace`
- `is_a`
- `not_a`
- `can_be`
- `cant_be`
- `has_part`
- `part_of`
- `inverse_of`
- `negates`

Seed edges:
- every relation kind `is_a Relation`
- `has_part inverse_of part_of`
- `part_of inverse_of has_part`
- `is_a negates not_a`
- `not_a negates is_a`
- `can_be negates cant_be`
- `cant_be negates can_be`

Rules:
- core kinds are explicit semantic facts, not inferred aliases
- v1 does not store a reverse-taxonomy relation for `is_a`
- reverse taxonomy traversal is query behavior, not stored semantics

## 5. Graph Model

The ontology is a directed graph of semantic triples.

Edge shape:

```rust
struct SemanticEdge {
    subject: Kind,
    relation: Kind,
    target: Kind,
    weight: Option<Weight>,
}
```

Edge identity:
- `(subject, relation, target)`
- weight is not part of identity

Weight:
- `Weight`
- used for optional relation metadata, not for identity
- unweighted edges are the default
- use `add_edge_weighted` only when the relation needs a scalar payload
- v1 uses one scalar type only; do not introduce a heterogeneous `EdgeWeight` enum in v1

Storage:
- canonical edge storage should be a dense SoA or equivalent compact edge table
- derived adjacency views should be built for fast scans
- avoid hash-map-only traversal paths on the hot path

Required read views:
- subject, relation -> targets
- relation, target -> subjects
- exact triple existence
- incoming and outgoing adjacency
- relation-filtered traversal

Fast-path helpers:
- `has_edge(subject, relation, target) -> bool`
- `targets(subject, relation) -> &[Kind]`
- `subjects(relation, target) -> &[Kind]`
- `neighbors(kind, relation, direction: EdgeDirection) -> Vec<Kind>`
- `reachable(kind, relation, direction: EdgeDirection, depth: usize) -> Vec<Kind>`
- `can_reach(start, relation, target, depth: usize) -> bool`
- `is_a(subject, target) -> bool`

Taxonomy semantics:
- `is_a` is the only dedicated taxonomy helper in v1
- transitive taxonomy queries are expressed through `reachable` or `TraversalQuery` over the `is_a` relation
- `reachable` is bounded traversal over any relation and `EdgeDirection`
- `can_reach` is the boolean form of `reachable`

## 6. Snapshots and Concurrency

Readers consume immutable snapshots.

A mutable `SemanticRegistry` stages changes and produces `SemanticSnapshot` values.

`SemanticSnapshot` is a method-based read model:
- its fields are private
- it does not expose raw lookup tables or adjacency storage directly
- it exposes direct helpers and query builders only

Rules:
- readers may share snapshots concurrently
- readers never mutate active semantic state
- a single writer stages commands and builds the next snapshot
- snapshot swap must be atomic from the reader point of view
- snapshot versions must change on any material semantic mutation
- caches are version-aware and tied to a snapshot version

Snapshot contents:
- kind registry data
- dense remap tables
- canonical edge storage
- adjacency views
- compiled-query metadata
- version metadata
- read-only derived caches

Snapshot API:
- `version() -> u64`
- `core() -> Core`
- `kinds() -> &[Kind]` in ascending internal dense order
- `edges() -> &[SemanticEdge]` in `(subject, relation, target)` order
- `kind(name) -> Option<Kind>`
- `name(kind) -> Option<&str>`
- `type_id(kind) -> Option<TypeId>`
- `has_edge(subject, relation, target) -> bool`
- `targets(subject, relation) -> &[Kind]`
- `subjects(relation, target) -> &[Kind]`
- `neighbors(kind, relation, direction: EdgeDirection) -> Vec<Kind>`
- `reachable(kind, relation, direction: EdgeDirection, depth: usize) -> Vec<Kind>`
- `can_reach(start, relation, target, depth: usize) -> bool`
- `is_a(subject, target) -> bool`
- `edge_query() -> EdgeQueryBuilder`
- `traversal_query() -> TraversalQueryBuilder`

Exact one-hop row lookups are zero-allocation borrowed views into the snapshot.
They are unique and sorted by ascending internal dense order.
Traversal helpers return deterministic, deduplicated results.

## 7. Mutation Model

Mutations are staged as commands.

Core commands:
- `RegisterKind { name: Arc<str> }`
- `UnregisterKind { kind: Kind }`
- `RegisterTypedKind { type_id: TypeId, name: Option<Arc<str>> }`
- `AddEdge { subject: Kind, relation: Kind, target: Kind, weight: Option<Weight> }`
- `RemoveEdge { subject: Kind, relation: Kind, target: Kind }`

Rules:
- command order is deterministic
- no mutation occurs through a read snapshot
- no implicit aliasing or correction
- invalid registrations and collisions return explicit errors
- unregistering a kind removes its incident edges and any type binding
- core kinds cannot be unregistered
- `AddEdge` upserts the triple and optional weight metadata
- `RemoveEdge` removes by triple only
- compiled queries and caches must become stale when versions change

## 8. Query Model

Public query types:
- `EdgeQuery`, `EdgeQueryBuilder`, `CompiledEdgeQuery`
- `TraversalQuery`, `TraversalQueryBuilder`, `CompiledTraversalQuery`

Compiled queries expose the same terminal methods as their builders and are bound to a specific snapshot version.

Builder creation:
- `SemanticSnapshot::edge_query() -> EdgeQueryBuilder`
- `SemanticSnapshot::traversal_query() -> TraversalQueryBuilder`
- `EdgeQuery::builder() -> EdgeQueryBuilder`
- `TraversalQuery::builder() -> TraversalQueryBuilder`

The `SemanticSnapshot::*_query()` constructors return builders already bound to that snapshot; the `builder()` constructors are snapshot-agnostic runtime builders.

Registry lookups are direct methods, not query objects.

`EdgeQueryBuilder` supports:
- `subject`, `subjects`
- `relation`, `relations`
- `target`, `targets`
- `outgoing`, `incoming`, `both`
- `weight_any`, `weight_missing`, `weight_present`, `weight_eq(weight)`, `weight_ne(weight)`, `weight_gt(weight)`, `weight_gte(weight)`, `weight_lt(weight)`, `weight_lte(weight)`, `weight_range(min, max)`
- `subjects_from`, `targets_from`
- `intersect_subjects_from`, `intersect_targets_from`
- `select_edges`, `select_subjects`, `select_targets`
- `compile(snapshot) -> CompiledEdgeQuery`
- `run_edges(snapshot) -> Vec<SemanticEdge>`
- `run_subjects(snapshot) -> Vec<Kind>`
- `run_targets(snapshot) -> Vec<Kind>`
- `run_exists(snapshot) -> bool`
- `run_count(snapshot) -> usize`

`TraversalQueryBuilder` supports:
- `seeds`
- `relations`
- `outgoing`, `incoming`, `both`
- `depth_1`, `depth_exact`, `depth_to`, `transitive`
- `include_start`, `dedup`, `stop_on_match`, `max_results`
- `compile(snapshot) -> CompiledTraversalQuery`
- `run_kinds(snapshot) -> Vec<Kind>`
- `run_exists(snapshot) -> bool`
- `run_count(snapshot) -> usize`

Range semantics:
- `weight_range(min, max)` is inclusive on both ends
- `depth_to(n)` is inclusive
- depth arguments are `usize` hop counts

Weight predicate inputs are `Weight` values, and missing weights are matched explicitly via `weight_missing`.

Neighbor lookup is a direct helper only in v1; there is no dedicated neighbor query type.

Composition rules:
- only set-producing queries may feed `subjects_from` / `targets_from`
- nested queries compile before the outer query
- count and bool results cannot be piped onward

Result ordering:
- edge results are deterministic and sorted by `(subject, relation, target)`
- non-traversal kind results are deterministic and sorted by ascending internal dense order
- traversal results are deterministic and preserve discovery order
- duplicate results are removed when `dedup = true`
- `run_count` counts the final terminal result after filtering and deduplication

Query options:

```rust
struct QueryOptions {
    include_start: bool,
    dedup: bool,
    stop_on_match: bool,
    max_results: Option<usize>,
}
```

Recommended defaults:
- `include_start = false`
- `dedup = true`
- `stop_on_match = false`
- `max_results = None`

Version safety:
- compiled queries are bound to the snapshot version they were compiled against
- a version mismatch invalidates cached results
- direct helper methods may recompile automatically
- explicit compiled-query execution may return a stale-query error

## 9. Caching

Cache only read-only derived results tied to a snapshot version.

Cache keys should be based on:
- normalized query fingerprint
- snapshot version

Good caches:
- exact triple existence
- direct edge pattern results
- compiled query fingerprints
- transitive `is_a` ancestors and descendants
- repeated set-producing subquery results

Rules:
- no cross-version result reuse
- old snapshot caches remain valid only for old readers
- cache invalidation must be deterministic

## 10. Bevy Integration

Bevy support is built into the default crate build.

Provide:
- `SemanticsPlugin`
- `Semantics`
- `SemanticEdit`
- `SemanticPlaybackQueue`
- `CommandsExt`

Systems:
- collect semantic commands during normal gameplay systems
- apply queued commands in a controlled write stage
- enqueue task-produced command batches for later playback
- read the current snapshot from the `Semantics` resource when needed

Rules:
- systems read immutable snapshots only
- no Bevy system mutates semantic state through an immutable resource
- compiled-query caches may live in `Local` or `Resource`
- `Semantics` owns the writer-facing registry and the cached read model
- `Semantics::edit()` returns the fluent edit chain, while direct `Semantics` writes return `Result`
- `SemanticPlaybackQueue` is for batches produced off-thread and replayed on the main thread
- `Semantics::snapshot()` returns the cached `Arc<SemanticSnapshot>` for query work
- `Semantics::core()` returns the seeded core handles

Convenience command examples:
- `register_kind("Tree")`
- `unregister_kind(tree)`
- `unregister_namespace("game")`
- `typed_kind::<Health>()`
- `typed_kind_named::<Health>("Health")`
- `semantics.add_edge(a, is_a, b).add_edge(c, is_a, d).expect("seed taxonomy")`
- `add_edge_weighted(a, priority, b, Weight::from(10))`
- `remove_edge(a, is_a, b)`

## 11. Serialization

Serde support is optional in v1.

Recommended serializable forms:
- `Kind` as canonical name or hex digest
- `Weight` as `i64`
- command batches
- query builders

Rules:
- compiled query serialization is optional
- portable formats should prefer canonical names over raw numeric IDs
- raw digest serialization is acceptable when the digest algorithm is frozen
- runtime-only typed-kind registration commands do not need portable serialization in v1

## 12. Testing

Tests must cover:
- stable kind generation
- collision detection
- typed kind registration
- core seed relations
- exact triple lookup
- unweighted edge storage and optional weight filtering
- edge weight upsert on repeated `AddEdge`
- forward and reverse adjacency
- transitive `is_a` traversal
- nested query composition
- stale compiled-query invalidation
- snapshot stability during writer swap
- Bevy command and apply flow
- task snapshot playback and queued command replay

## 13. Benchmarks

Benchmarks should cover:
- name -> id
- id -> name
- `has_edge`
- `targets(subject, relation)`
- `subjects(relation, target)`
- cached `is_a`
- builder execution vs compiled execution
- nested query materialization
- snapshot swap cost

Benchmark graph profiles:
- tiny
- medium
- large taxonomy-heavy
- large mixed-relation

## 14. Module Layout

Suggested crate layout:
- `kind.rs`
- `weight.rs`
- `registry.rs`
- `graph.rs`
- `snapshot.rs`
- `command.rs`
- `query/edge.rs`
- `query/traversal.rs`
- `query/options.rs`
- `query/compiled.rs`
- `bevy.rs`
- `error.rs`
- `prelude.rs`

## 15. Feature Flags

No Cargo feature flags are defined in v1.

If optional extensions are added later, they must not change the default API surface or make the Bevy integration opt-in again.

## 16. Non-Goals

Do not include in v1:
- synonym or alias frameworks
- fuzzy semantic matching
- text search
- user-authored semantic DSL
- full graph-database optimizer
- mutable shared read/write graph
- stored reverse relation for `is_a`
- automatic semantic inference engine
- weighted pathfinding heuristics beyond basic filtering

## 17. V1 Summary

v1 is:
- stable public `Kind(u64)`
- optional typed kinds
- dense internal remapping
- explicit semantic registry and graph storage
- immutable reader snapshots
- staged single-writer mutation
- builder-based queries with compiled execution
- versioned caches
- optional Bevy integration
