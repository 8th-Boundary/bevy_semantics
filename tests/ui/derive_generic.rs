use bevy_ecs::component::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "Container")]
struct Container<T>(T);

fn main() {}
