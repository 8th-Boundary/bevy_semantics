//! Compile-time kind registration without Bevy.

use bevy_semantics::core::{IS_A, RELATION};
use bevy_semantics::{
    kind, semantic_kinds, Kind, KindRegistration, SemanticError, SemanticRegistry,
};

const CREATURE: Kind = kind!("Creature");

semantic_kinds! {
    const ANIMAL_KINDS = {
        WOLF = "Wolf",
        RABBIT = "Rabbit",
        PREYS_ON = "preys_on",
    };
}

fn main() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();

    // A single compile-time `Kind` keeps its canonical name as a separate argument.
    registry.register_const("Creature", CREATURE)?;

    // The macro-generated group is a borrowed tuple slice; no additional `&`
    // or runtime collection is required.
    let registrations: &[KindRegistration] = ANIMAL_KINDS;
    registry.register_consts(registrations)?;

    registry.add_edges([
        (PREYS_ON, IS_A, RELATION),
        (WOLF, IS_A, CREATURE),
        (RABBIT, IS_A, CREATURE),
        (WOLF, PREYS_ON, RABBIT),
    ])?;

    let snapshot = registry.snapshot();
    assert_eq!(snapshot.name(CREATURE), Some("Creature"));
    assert_eq!(snapshot.targets(WOLF, PREYS_ON), &[RABBIT]);

    println!("wolf prey: {:?}", snapshot.name(RABBIT));
    Ok(())
}
