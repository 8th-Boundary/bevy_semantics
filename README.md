# bevy_semantics

`bevy_semantics` adds a semantic layer to Bevy: stable kinds, labeled relations, immutable snapshots, and cached graph queries.

Semantics is the meaning layer above raw ECS data. An ontology graph is the directed labeled graph that stores that meaning: kinds are nodes, relations are edge labels, and edges connect those labels into a graph you can query.

Use this crate when you want to ask questions like:
- what kind is this?
- what does it inherit from?
- what does it prey on?
- what does it drop?
- where does it grow?

The crate is organized around a few small pieces:
- `SemanticRegistry` for mutable authoring
- `Semantics` for the Bevy-facing resource
- `SemanticEdit` for fluent bulk edits
- `SemanticSnapshot` for immutable reads and background work
- `SemanticPlaybackQueue` for off-thread command playback
- `CommandsExt` for staging semantic commands from Bevy systems

## Core Model

### Kind

A `Kind` is a stable semantic identity.

- It is an opaque public ID, not a string.
- It is derived from a canonical name.
- It is cheap to copy and safe to store in game data.
- `DenseKind` is internal only and never appears in the public API.

In practice, a kind is the semantic handle for a concept like `Wolf`, `preys_on`, or `Forest`.

### Relation

A relation is the label on an edge.

- In storage, a relation is also a `Kind`.
- The builtin `Relation` kind marks relation kinds as part of the schema.
- Custom relations are just normal kinds that you register and then use as labels.

For example, `preys_on` is a relation kind, and `Wolf preys_on Rabbit` is an edge that uses it.

### Edge

An edge is a semantic triple:

```rust
struct SemanticEdge {
    subject: Kind,
    relation: Kind,
    target: Kind,
    weight: Option<Weight>,
}
```

- `subject`, `relation`, and `target` identify the edge.
- `weight` is optional.
- Unweighted edges are the default.
- `Weight` is a signed `i64` newtype for scalar payloads.

### Storage

The registry owns the mutable source of truth.

- `SemanticRegistry` stores names, typed bindings, and canonical edge data.
- `SemanticSnapshot` turns that data into a read-optimized, immutable view.
- The snapshot uses internal dense indices and adjacency views for fast reads.
- Public code never sees the dense index type directly.

The short version:
- mutate through the registry
- read through the snapshot or the `Semantics` facade
- keep dense storage hidden behind the API

## API At A Glance

### `SemanticRegistry`

Use this when you are authoring outside Bevy or building setup code directly.

Main methods:
- `register_kind(name)`
- `register_typed_kind::<T>()`
- `register_typed_kind_named::<T>(name)`
- `unregister_kind(kind)`
- `add_edge(subject, relation, target)`
- `add_edge_weighted(subject, relation, target, weight)`
- `remove_edge(subject, relation, target)`
- `batch()`
- `apply_commands(...)`
- `ensure_builtins()`
- `snapshot()`
- `snapshot_cached()`

`batch()` is for bulk authoring when you want a write-heavy pass to commit once at the end.

### `Semantics`

Use this as the Bevy resource.

- `Res<Semantics>` is the read side.
- `ResMut<Semantics>` is the write side.
- Direct reads live here: `kind`, `kind_of`, `name`, `builtins`.
- Graph reads live here too: `is_a`, `targets`, `subjects`, `neighbors`, `reachable`, `can_reach`.
- Direct writes live here as well: `register_kind`, `register_typed_kind`, `register_typed_kind_named`, `unregister_kind`, `add_edge`, `add_edge_weighted`, `remove_edge`.
- `snapshot()` returns the cached immutable read model.

Tiny write sketch:

```rust
let creature = semantics.register_kind("Creature")?;
let wolf = semantics.register_typed_kind_named::<Wolf>("Wolf")?;
let preys_on = semantics.register_kind("preys_on")?;
semantics.add_edge(wolf, preys_on, rabbit)?;
```

### `SemanticEdit`

Use this for fluent bulk authoring when you want one chained write pass.

It supports:
- `register_kind`
- `register_typed_kind`
- `register_typed_kind_named`
- `unregister_kind`
- `add_edge`
- `add_edge_weighted`
- `remove_edge`
- `expect(...)`
- `finish()`

Example shape:

```rust
semantics
    .edit()
    .register_kind("preys_on")
    .register_kind("predated_by")
    .add_edge_weighted(wolf, damage, rabbit, Weight::from(7))
    .expect("seed semantic schema");
```

### `SemanticSnapshot`

Use this for stable read-only work.

- It can be cloned into background tasks.
- It is the right surface for compiled queries and longer-lived read work.
- It is immutable once created.

Call `semantics.snapshot()` when you need a cloned read model.

### `SemanticPlaybackQueue`

Use this when a background task produces semantic commands that should be replayed later on the main thread.

- A task can read a snapshot.
- The task can derive `SemanticCommand` values.
- A main-thread system can enqueue and play them back later.

See `examples/task_playback.rs` for the full flow.

### `CommandsExt`

Use this when you want to stage semantic edits from ordinary Bevy systems.

- It queues semantic commands through Bevy `Commands`.
- The plugin drains and applies them later.
- It is the easiest way to keep semantic authoring inside a Bevy schedule.

## Builtins

The registry seeds a small set of canonical kinds and relations automatically.

Read them from `semantics.builtins()` or `ensure_builtins()`.

| Builtin | Use case |
| --- | --- |
| `Relation` | Meta-kind for relation kinds themselves. Use it to make custom relations part of the schema. |
| `Namespace` | Reserved grouping kind for names and ownership. Useful for plugin, module, and content boundaries. |
| `is_a` | Taxonomy, inheritance, and required-component style hierarchies. |
| `not_a` | Explicit exclusion or disjoint categories. |
| `can_be` | Positive capability, affordance, or allowed classification. |
| `cant_be` | Negative capability or disallowed classification. |
| `has_part` | Composition from whole to part. |
| `part_of` | Inverse composition from part to whole. |
| `inverse_of` | Declare that two relations are inverses. |
| `negates` | Declare logical opposites such as `is_a` vs `not_a`. |

In practice, you usually seed builtins once and keep the returned handles around.

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

These custom relations are not builtins. Register them like any other kind, mark them `is_a Relation`, and couple the pairs with `inverse_of`.

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

The crate includes Criterion benches under `bevy_semantics/benches/semantics_bench.rs` for:

- snapshot build
- cached snapshot retrieval
- bulk authoring
- kind lookup
- direct taxonomy checks
- dynamic and compiled edge queries
- dynamic and compiled traversal queries
- task-style command derivation
- queued command playback

## Notes

- `Kind` and `Weight` are opaque newtypes, not type aliases.
- Relations are unweighted by default. Use weighted edges only when the scalar matters.
- Built-in kinds are protected. `unregister_kind` removes user kinds, their incident edges, and any typed binding, but built-ins cannot be removed.
- The crate favors deterministic results and cached reads over write-side sophistication.
