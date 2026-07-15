# bevy_semantics

`bevy_semantics` is a high-level semantic layer for Bevy and Rust projects.
It maps abstract concepts to stable kinds and records how those concepts relate.
It is not the definition of a thing, and it is not a runtime instance of a thing.

An ontology graph is the directed graph that stores that meaning:
- kinds are the nodes
- relations are kinds used as edge labels
- edges connect a subject kind, relation kind, and target kind

Use this crate when you want to answer questions like:
- what is this concept?
- what does it inherit from?
- what does it prey on?
- what does it drop?
- where does it grow?

## Bevy Compatibility

| bevy_semantics | Bevy |
| --- | --- |
| `0.2.x` | `0.19.x` |
| `0.1.x` | `0.18.x` |

## What This Crate Gives You

- stable semantic identities
- optional typed kinds
- a read-optimized ontology graph
- immutable snapshots for concurrent reads
- query compilation and caching
- built-in Bevy integration in the default build

## Core Ideas

### Kind

A `Kind` is a stable semantic identifier for a concept.

- it is opaque, not a string
- it is derived from a canonical name
- it is cheap to copy and safe to store in game data
- it names a concept, not an entity instance

Examples:
- `Wolf`
- `preys_on`
- `Forest`

Static kinds can be declared without a registry or a Bevy world:

```rust
use bevy_semantics::{kind, Kind};

pub const MOVE_TO: Kind = kind!("MoveTo");
```

`kind!` trims the literal and performs the stable, domain-separated BLAKE3
calculation during macro expansion. The generated program contains the completed
`u64`; declaring the constant does not register a name or Rust type at runtime.

### Semantic Components

A semantic component binds a Bevy component type to a static `Kind`:

```rust
use bevy_ecs::prelude::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "Health")]
struct Health {
    current: f32,
}
```

`Health::KIND` and `Health::KIND_NAME` are available without a registry or
world. Installing `SemanticsPlugin` creates the world-local mapping resource,
and explicit registration can prewarm the binding:

```rust
use bevy_semantics::{SemanticAppExt, SemanticsPlugin};

# let mut app = bevy_app::App::new();
app.add_plugins(SemanticsPlugin)
    .register_semantic_component::<Health>();
```

Bevy `ComponentId` values are world-local, so they are never part of `Kind`, a
snapshot, or reusable data. A registered component binding remains for the
world's lifetime.

Open generic definitions need a stable identity for each concrete
monomorphization. Declare those explicitly:

```rust
use bevy_ecs::prelude::Component;
use bevy_semantics::semantic_component;

#[derive(Component)]
struct Container<T>(T);

semantic_component!(Container<u32>, kind = "Container<u32>");
semantic_component!(Container<String>, kind = "Container<String>");
```

This keeps `Container<u32>` and `Container<String>` one-to-one with their own
`TypeId`, `Kind`, and per-world `ComponentId`.

Behavior derives can normally generate this implementation themselves. If a
type intentionally lists both derives, add the explicit `standalone` marker so
the behavior derive defers to this derive:

```rust
#[derive(Component, Behavior, SemanticComponent)]
#[semantic(kind = "MoveTo", standalone)]
struct MoveTo {
    #[behavior_state]
    state: BehaviorState,
}
```

### Relation

A relation is just a kind used in the relation slot of an edge.

- relations are not a separate storage type
- relation kinds are normal kinds
- the core `Relation` kind marks kinds that are intended for use as edge labels

In other words, `preys_on` is both a kind and a relation label.

### Edge

An edge is a semantic triple, with optional scalar metadata:

```rust
struct SemanticEdge {
    subject: Kind,
    relation: Kind,
    target: Kind,
    weight: Option<Weight>,
}
```

- `subject`, `relation`, and `target` identify the edge
- `weight` is optional
- unweighted edges are the default
- `Weight` is a signed `i64` newtype

### Storage Model

The crate splits mutation and read access:

- `SemanticRegistry` is the mutable source of truth
- `SemanticSnapshot` is the immutable read model
- `Semantics` is the Bevy-facing resource that wraps both

The snapshot is built from the registry and uses dense internal indexing for reads.
Public code never sees the dense index type.

## API Overview

### `SemanticRegistry`

Use this when authoring outside Bevy or in setup code.

Main methods:
- `register_kind(name)`
- `typed_kind::<T>()`
- `typed_kind_named::<T>(name)`
- `unregister_kind(kind)`
- `tombstone_kind(kind)`
- `revive_kind(kind)`
- `unregister_namespace(namespace)`
- `add_edge(subject, relation, target)`
- `add_edge_weighted(subject, relation, target, weight)`
- `remove_edge(subject, relation, target)`
- `batch()`
- `apply_commands(commands)`
- `core()`
- `kind(name)`
- `kind_of::<T>()`
- `name(kind)`
- `type_id(kind)`
- `snapshot()`
- `snapshot_cached()`

`batch()` is for bulk authoring when you want to stage many writes and rebuild once at the end.

Kinds pinned by static components cannot be unregistered because their Bevy
component registration cannot be undone. They may be tombstoned instead.
Tombstoning removes incident ontology edges and excludes the kind from
snapshots while preserving its stable name and Rust type binding. Explicit
`register_kind` or `revive_kind` reactivates it; component use alone does not.

### `Semantics`

Use this as `Res<Semantics>` or `ResMut<Semantics>` in Bevy.

- `Res<Semantics>` is the read side
- `ResMut<Semantics>` is the write side
- direct reads live here: `kind`, `kind_of`, `name`, `core`
- graph reads live here too: `is_a`, `targets`, `subjects`, `neighbors`, `reachable`, `can_reach`, `has_edge`
- direct writes live here: `register_kind`, `typed_kind`, `typed_kind_named`, `unregister_kind`, `tombstone_kind`, `revive_kind`, `unregister_namespace`, `add_edge`, `add_edge_weighted`, `remove_edge`
- `snapshot()` returns the cached immutable read model for background work

Example shape:

```rust
semantics
    .edit()
    .register_kind("preys_on")
    .register_kind("predated_by")
    .typed_kind_named::<Wolf>("Wolf")
    .add_edge(wolf, preys_on, rabbit)
    .add_edge_weighted(wolf, damage, rabbit, Weight::from(7))
    .expect("seed semantic schema");
```

### `SemanticEdit`

Use this for fluent bulk authoring.

It supports:
- `register_kind`
- `typed_kind`
- `typed_kind_named`
- `unregister_kind`
- `tombstone_kind`
- `revive_kind`
- `unregister_namespace`
- `add_edge`
- `add_edge_weighted`
- `remove_edge`
- `expect(...)`
- `finish()`

### `SemanticSnapshot`

Use this for stable read-only work.

- it is immutable
- it can be cloned into background tasks
- it is the right surface for compiled queries
- direct reads do not need a snapshot
- background tasks do

Call `semantics.snapshot()` when you need a cloned read model.

### `SemanticPlaybackQueue`

Use this when a background task produces semantic commands that should be replayed later on the main thread.

- a task reads a snapshot
- the task derives `SemanticCommand` values
- a main-thread system enqueues them
- the plugin replays them later

See `examples/task_playback.rs` for the full flow.

### `CommandsExt`

Use this when you want to stage semantic edits from ordinary Bevy systems.

- it queues semantic commands through Bevy `Commands`
- the plugin drains and applies them later
- it is the easiest way to keep semantic authoring inside a Bevy schedule

## Core

The crate seeds a small set of core kinds automatically.
User code does not need to call an "ensure core" function.

The crate reserves the `Core` namespace for itself, but the seeded core kinds use plain names:

| Core kind | Use case |
| --- | --- |
| `Relation` | Marks relation kinds as part of the schema |
| `Namespace` | Reserved grouping kind for names and ownership |
| `is_a` | Taxonomy, inheritance, and required-component style hierarchies |
| `not_a` | Explicit exclusion or disjoint categories |
| `can_be` | Positive capability, affordance, or allowed classification |
| `cant_be` | Negative capability or disallowed classification |
| `has_part` | Composition from whole to part |
| `part_of` | Inverse composition from part to whole |
| `inverse_of` | Declare that two relations are inverses |
| `negates` | Declare logical opposites such as `is_a` vs `not_a` |

Core facts are seeded automatically:
- every relation kind `is_a Relation`
- `has_part inverse_of part_of`
- `part_of inverse_of has_part`
- `is_a negates not_a`
- `not_a negates is_a`
- `can_be negates cant_be`
- `cant_be negates can_be`

The `Core` namespace is reserved and cannot be removed, in any case variant.

## Example World

The runnable examples use a small game-world-like ecosystem:

- `Creature -> Beast -> Canine -> Wolf`
- `Creature -> Beast -> Rabbit`
- `Item -> Resource -> Herb`
- `Item -> Loot -> Pelt`
- `Place -> Forest`

Custom demo relations:
- `preys_on` / `predated_by`
- `drops` / `dropped_by`
- `grows_in`

These custom relations are not core kinds.
Register them like any other kind, optionally classify them under `Relation`, and couple the paired relations with `inverse_of`.

If you want a full end-to-end example, see `examples/basic.rs`.

## Background Tasks

`SemanticSnapshot` is designed to be cloned into background work.

The usual flow is:
- read a snapshot on a task
- derive a batch of `SemanticCommand` values
- enqueue them in `SemanticPlaybackQueue`
- replay them later on the main thread

That keeps background work deterministic and keeps the mutable registry on the main thread.

If you want a runnable example, see `examples/task_playback.rs`.

## When A Relation Should Be An Entity

Use an entity for a relation when:
- the relation has its own component data
- the relation needs spawn/despawn lifecycle
- the relation must be queried like any other ECS object

In that case, keep the semantic relation kind as the stable label and let the entity carry the runtime data.

## Benchmarks

### Hardware and Tooling

- CPU: 13th Gen Intel Core i7-13700K
- cores: 16 physical, 24 logical
- memory: 32.0 GiB
- OS: Windows 11 Pro 64-bit, build 26200
- Rust: `rustc 1.91.1 (ed61e7d7e 2025-11-07)`
- target: `x86_64-pc-windows-msvc`

### Latest Criterion Results

Measured with `cargo bench -p bevy_semantics --bench semantics_bench` on the machine above after the API cleanup pass.

| Benchmark | Time |
| --- | --- |
| `semantics_snapshot_build_4k` | `1.0560 ms - 1.0873 ms` |
| `semantics_snapshot_cached_4k` | `16.301 ns - 16.567 ns` |
| `semantics_bulk_authoring_4k` | `2.0926 ms - 2.1538 ms` |
| `semantics_lookup_kind_by_name` | `6.1803 ns - 6.4976 ns` |
| `semantics_is_a_direct` | `34.189 ns - 34.429 ns` |
| `semantics_edge_query_dynamic` | `47.730 ns - 48.824 ns` |
| `semantics_edge_query_compiled` | `34.345 ns - 35.166 ns` |
| `semantics_traversal_dynamic` | `52.202 ns - 54.122 ns` |
| `semantics_traversal_compiled` | `37.500 ns - 37.934 ns` |
| `semantics_task_command_derivation_4k` | `283.97 ns - 287.82 ns` |
| `semantics_task_command_playback_4k` | `37.739 us - 40.944 us` |

## Notes

- `Kind` and `Weight` are opaque newtypes, not type aliases.
- Relations are unweighted by default. Use weighted edges only when the scalar matters.
- Core kinds are protected. `unregister_kind` removes ordinary user kinds, their incident edges, and any typed binding. Static component kinds are identity-pinned and must be tombstoned instead.
- The crate favors deterministic results and cached reads over write-side sophistication.
