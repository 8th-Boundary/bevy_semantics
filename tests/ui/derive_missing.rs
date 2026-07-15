use bevy_ecs::component::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
struct Missing;

fn main() {}
