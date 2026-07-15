//! Runtime/manual kind registration without Bevy or compile-time declarations.

use bevy_semantics::{SemanticError, SemanticRegistry};

fn main() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let core = registry.core();

    // This is the dynamic path: each canonical name is converted to a Kind and
    // registered at runtime. Prefer `const WOLF: Kind = kind!("Wolf")` when a
    // name is known while compiling the program.
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
