use core::any::TypeId;

use bevy_app::App;
use bevy_ecs::component::Component;
use bevy_semantics::{
    kind, semantic_component, Kind, SemanticAppExt, SemanticComponent, SemanticComponents,
    SemanticError, SemanticRegistry, SemanticWorldExt, Semantics, SemanticsPlugin,
};

const MOVE_TO: Kind = kind!("MoveTo");
static ATTACK: Kind = kind!("Attack");

#[derive(Component, SemanticComponent)]
#[semantic(kind = "  Health  ")]
struct Health;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "Standalone", standalone)]
struct Standalone;

#[derive(Component)]
struct Container<T>(T);

semantic_component!(Container<u32>, kind = "Container<u32>");
semantic_component!(Container<String>, kind = "Container<String>");

#[derive(Component)]
struct ConflictingHealth;

impl SemanticComponent for ConflictingHealth {
    const KIND: Kind = Health::KIND;
    const KIND_NAME: &'static str = Health::KIND_NAME;
}

#[derive(Component)]
struct InvalidStaticKind;

impl SemanticComponent for InvalidStaticKind {
    const KIND: Kind = kind!("NotInvalidStaticKind");
    const KIND_NAME: &'static str = "InvalidStaticKind";
}

#[test]
fn kind_macro_is_const_and_matches_runtime_protocol() -> Result<(), SemanticError> {
    assert_eq!(kind!("MoveTo").as_u64(), 8_061_483_968_094_997_415);
    assert_eq!(kind!("Attack").as_u64(), 8_616_870_665_771_427_190);
    assert_eq!(kind!("game::Health").as_u64(), 16_591_017_121_866_416_838);
    assert_eq!(kind!("snake_case").as_u64(), 2_623_833_307_385_135_211);
    assert_ne!(MOVE_TO, ATTACK);
    assert_eq!(kind!("MoveTo"), kind!("  MoveTo  "));

    let mut registry = SemanticRegistry::default();
    for name in ["MoveTo", "Attack", "game::Health", "snake_case"] {
        let runtime = registry.register_kind(name)?;
        let compile_time = match name {
            "MoveTo" => kind!("MoveTo"),
            "Attack" => kind!("Attack"),
            "game::Health" => kind!("game::Health"),
            "snake_case" => kind!("snake_case"),
            _ => unreachable!(),
        };
        assert_eq!(compile_time, runtime);
    }
    Ok(())
}

#[test]
fn derive_and_concrete_generic_macro_expose_static_identity() {
    assert_eq!(Health::KIND_NAME, "Health");
    assert_eq!(Health::KIND, kind!("Health"));
    assert_eq!(Container::<u32>::KIND_NAME, "Container<u32>");
    assert_eq!(Container::<u32>::KIND, kind!("Container<u32>"));
    assert_ne!(Container::<u32>::KIND, Container::<String>::KIND);
    assert_eq!(Standalone::KIND, kind!("Standalone"));
}

#[test]
fn explicit_registration_is_bidirectional_and_idempotent() {
    let mut app = App::new();
    app.add_plugins(SemanticsPlugin)
        .register_semantic_component::<Health>()
        .register_semantic_component::<Health>();

    let component_id = app
        .world()
        .component_id::<Health>()
        .expect("Health should be registered with Bevy");
    let mappings = app.world().resource::<SemanticComponents>();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings.component_id(Health::KIND), Some(component_id));
    assert_eq!(mappings.component_id_of::<Health>(), Some(component_id));
    assert_eq!(mappings.kind(component_id), Some(Health::KIND));

    let semantics = app.world().resource::<Semantics>();
    assert_eq!(semantics.kind(Health::KIND_NAME), Ok(Health::KIND));
    assert_eq!(semantics.kind_of::<Health>(), Ok(Health::KIND));
    assert_eq!(
        semantics.type_id(Health::KIND),
        Some(TypeId::of::<Health>())
    );
}

#[test]
fn registration_works_without_plugin_and_plugin_can_follow() {
    let mut app = App::new();
    app.world_mut()
        .try_register_semantic_component::<Health>()
        .expect("registration should initialize direct dependencies");
    assert!(app.world().contains_resource::<Semantics>());
    assert!(app.world().contains_resource::<SemanticComponents>());

    app.add_plugins(SemanticsPlugin);
    assert_eq!(app.world().resource::<SemanticComponents>().len(), 1);
}

#[test]
fn static_mismatch_and_kind_conflicts_are_reported_before_insertion() {
    let mut world = bevy_ecs::world::World::new();
    let mismatch = world
        .try_register_semantic_component::<InvalidStaticKind>()
        .expect_err("manual identity mismatch should fail");
    assert!(matches!(mismatch, SemanticError::StaticKindMismatch { .. }));
    assert!(world.component_id::<InvalidStaticKind>().is_none());

    world
        .try_register_semantic_component::<Health>()
        .expect("first binding should succeed");
    let conflict = world
        .try_register_semantic_component::<ConflictingHealth>()
        .expect_err("one kind cannot bind two components");
    assert!(matches!(
        conflict,
        SemanticError::KindComponentConflict { .. }
    ));
    assert!(world
        .get::<ConflictingHealth>(bevy_ecs::entity::Entity::PLACEHOLDER)
        .is_none());
}

#[test]
fn separate_worlds_keep_independent_component_ids() {
    #[derive(Component)]
    struct Perturbation;

    let mut first = bevy_ecs::world::World::new();
    let mut second = bevy_ecs::world::World::new();
    second.register_component::<Perturbation>();

    let first_id = first
        .try_register_semantic_component::<Health>()
        .expect("first world registration");
    let second_id = second
        .try_register_semantic_component::<Health>()
        .expect("second world registration");

    assert_ne!(first_id, second_id);
    assert_eq!(
        first.resource::<SemanticComponents>().kind(first_id),
        Some(Health::KIND)
    );
    assert_eq!(
        second.resource::<SemanticComponents>().kind(second_id),
        Some(Health::KIND)
    );
}

#[test]
fn static_component_kinds_are_pinned_and_can_be_tombstoned() {
    let mut world = bevy_ecs::world::World::new();
    world
        .try_register_semantic_component::<Health>()
        .expect("registration should succeed");

    let error = world
        .resource_mut::<Semantics>()
        .unregister_kind(Health::KIND)
        .expect_err("pinned identities cannot be removed");
    assert!(matches!(
        error,
        SemanticError::CannotUnregisterStaticComponentKind { .. }
    ));

    assert!(world
        .resource_mut::<Semantics>()
        .tombstone_kind(Health::KIND)
        .expect("tombstone should succeed"));
    let semantics = world.resource::<Semantics>();
    assert_eq!(semantics.is_tombstoned(Health::KIND), Some(true));
    assert_eq!(semantics.kind(Health::KIND_NAME), Ok(Health::KIND));
    assert_eq!(semantics.kind_of::<Health>(), Ok(Health::KIND));
    assert!(semantics.snapshot().kind(Health::KIND_NAME).is_none());

    world
        .try_register_semantic_component::<Health>()
        .expect("binding fast path must preserve the tombstone");
    assert_eq!(
        world.resource::<Semantics>().is_tombstoned(Health::KIND),
        Some(true)
    );

    assert_eq!(
        world
            .resource_mut::<Semantics>()
            .register_kind(Health::KIND_NAME),
        Ok(Health::KIND)
    );
    assert_eq!(
        world.resource::<Semantics>().is_tombstoned(Health::KIND),
        Some(false)
    );
}
