//! The older runtime/manual path without compile-time declarations.

use bevy_semantics::{SemanticError, SemanticRegistry};

fn main() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let core = registry.core();

    // This intentionally avoids `kind!`, `KindRegistration`, and
    // `semantic_kinds!` to demonstrate names discovered only at runtime.
    let creature = registry.register_kind("Creature")?;
    let wolf = registry.register_kind("Wolf")?;
    let preys_on = registry.register_kind("preys_on")?;
    let rabbit = registry.register_kind("Rabbit")?;

    registry.add_edge(preys_on, core.is_a, core.relation)?;
    registry.add_edge(wolf, core.is_a, creature)?;
    registry.add_edge(wolf, preys_on, rabbit)?;

    let snapshot = registry.snapshot();
    let prey = snapshot.targets(wolf, preys_on);
    let prey_names: Vec<_> = prey
        .iter()
        .filter_map(|kind| registry.name(*kind))
        .collect();

    println!("wolf prey: {prey_names:?}");
    Ok(())
}
