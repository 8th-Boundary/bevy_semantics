use bevy_app::App;
use bevy_ecs::prelude::Component;
use bevy_semantics::prelude::*;

const HEALTH: Kind = kind!("Health");
const HEALTH_CONTAINER: Kind = kind!("Container<Health>");

#[derive(Component, SemanticComponent)]
#[semantic(kind = "Health")]
struct Health {
    _current: f32,
}

#[derive(Component)]
struct Container<T>(T);

semantic_component!(Container<Health>, kind = "Container<Health>");

fn main() {
    assert_eq!(HEALTH, Health::KIND);
    assert_eq!(HEALTH_CONTAINER, Container::<Health>::KIND);

    let mut app = App::new();
    // Component registration publishes each type's `KIND_NAME` automatically,
    // so semantic components do not need a separate `KindRegistration` slice.
    app.add_plugins(SemanticsPlugin)
        .register_semantic_component::<Health>()
        .register_semantic_component::<Container<Health>>();

    let mappings = app.world().resource::<SemanticComponents>();
    assert!(mappings.component_id_of::<Health>().is_some());
    assert!(mappings.component_id_of::<Container<Health>>().is_some());

    let semantics = app.world().resource::<Semantics>();
    assert_eq!(semantics.name(HEALTH), Some("Health"));
    assert_eq!(semantics.name(HEALTH_CONTAINER), Some("Container<Health>"));
}
