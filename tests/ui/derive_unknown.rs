use bevy_ecs::component::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
#[semantic(name = "Unknown")]
struct Unknown;

fn main() {}
