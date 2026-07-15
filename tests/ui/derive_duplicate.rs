use bevy_ecs::component::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "One")]
#[semantic(kind = "Two")]
struct Duplicate;

fn main() {}
