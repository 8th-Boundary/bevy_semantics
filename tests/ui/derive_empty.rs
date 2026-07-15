use bevy_ecs::component::Component;
use bevy_semantics::SemanticComponent;

#[derive(Component, SemanticComponent)]
#[semantic(kind = "   ")]
struct Empty;

fn main() {}
