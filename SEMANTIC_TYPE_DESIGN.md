# Semantic Type Design

> Implementation status: implemented for the upcoming `0.3.0` release.

This document records the `bevy_semantics` changes needed by typed consumers
such as `bevy_cognition`. They belong to `bevy_semantics`; cognition should use
the resulting contracts without owning their implementation.

## Goal

Provide stable semantic identity for Rust values that are not Bevy components,
while preserving the existing world-local mapping for component types.

Examples include memory datums, HTN marker types, conditions, tasks, and values
stored inside typed collection components.

## General semantic identity

Introduce a general trait:

```rust
pub trait SemanticType: Send + Sync + 'static {
    const KIND: Kind;
    const KIND_NAME: &'static str;
}
```

`#[derive(SemanticType)]` generates the implementation from
`#[semantic(kind = "...")]` and performs the same stable-name validation as
`SemanticComponent`.

Concrete generic instantiations use
`semantic_type!(ConcreteType, kind = "CanonicalName")`.

`SemanticComponent` becomes the component-specific refinement:

```rust
pub trait SemanticComponent: Component + SemanticType {}
```

`#[derive(SemanticComponent)]` should provide both contracts so existing
component authoring remains concise. Manual implementations remain supported.
An existing manual implementation moves `KIND` and `KIND_NAME` onto its
`SemanticType` implementation and uses an empty `SemanticComponent`
implementation for the component-specific marker.

## Type, kind, and component mappings

The registry retains the distinctions between:

```text
TypeId
    Exact in-process Rust type identity

Kind
    Stable semantic and persistence identity

ComponentId
    World-local Bevy storage identity
```

Registration must validate one-to-one `TypeId` and `Kind` bindings. Component
registration additionally binds the current world's `ComponentId`.

## Parameterized storage remains consumer-owned

A concrete generic storage type does not automatically become a new semantic
concept. For example, `Memories<KnownFoodSource>` may be described by its
consumer using two atomic Kinds:

```text
Collection family
    Memories

Contained datum
    KnownFoodSource
```

`bevy_semantics` should not synthesize a compound collection `Kind` merely to
mirror a Rust generic instantiation. The consumer may retain the concrete
`TypeId` and world-local `ComponentId` in its own storage descriptor while
persisting the structured atomic-Kind pair.

If a concrete generic wrapper genuinely represents a separately named ontology
concept, its author may give that wrapper an explicit `SemanticComponent`
identity. Compiler-generated Rust type names are never persistence contracts.

## Compatibility

The change should preserve the concise existing component path:

```rust
#[derive(Component, SemanticComponent)]
#[semantic(kind = "Health")]
pub struct Health(pub f32);
```

Non-component values gain the parallel path:

```rust
#[derive(SemanticType)]
#[semantic(kind = "KnownFoodSource")]
pub struct KnownFoodSource {
    // ...
}
```

This work should land in `bevy_semantics` before cognition relies on
`SemanticType` as a public bound.
