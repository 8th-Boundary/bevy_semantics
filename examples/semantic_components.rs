use bevy_app::App;
use bevy_ecs::prelude::Component;
use bevy_semantics::prelude::*;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "Health")]
struct Health {
    _current: f32,
}

#[derive(Component)]
struct Container<T>(T);

semantic_component!(Container<Health>, kind = "Container<Health>");

fn main() {
    const HEALTH: Kind = kind!("Health");
    assert_eq!(HEALTH, Health::KIND);

    let mut app = App::new();
    app.add_plugins(SemanticsPlugin)
        .register_semantic_component::<Health>()
        .register_semantic_component::<Container<Health>>();

    let mappings = app.world().resource::<SemanticComponents>();
    assert!(mappings.component_id_of::<Health>().is_some());
    assert!(mappings.component_id_of::<Container<Health>>().is_some());
}
