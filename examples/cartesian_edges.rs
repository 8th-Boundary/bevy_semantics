use bevy_semantics::core::{IS_A, RELATION};
use bevy_semantics::{cartesian_edges, semantic_kinds, SemanticError, SemanticRegistry};

semantic_kinds! {
    const EXAMPLE_KINDS = {
        PLAYER = "Player",
        GUIDE = "Guide",
        CAN_ENTER = "can_enter",
        CAN_LEAVE = "can_leave",
        FOREST = "Forest",
        MEADOW = "Meadow",
    };
}

fn main() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    registry.register_consts(EXAMPLE_KINDS)?;
    registry.add_edges([(CAN_ENTER, IS_A, RELATION), (CAN_LEAVE, IS_A, RELATION)])?;

    // Both actors can enter and leave both places: 2 actors × 2 permissions × 2 places.
    let combinations = cartesian_edges([PLAYER, GUIDE], [CAN_ENTER, CAN_LEAVE], [FOREST, MEADOW]);
    assert_eq!(combinations.len(), 8);
    registry.add_edges(combinations)?;

    let snapshot = registry.snapshot();
    let player_permissions = snapshot
        .edge_query()
        .cartesian(PLAYER, [CAN_ENTER, CAN_LEAVE], [FOREST, MEADOW])
        .run_edges(&snapshot)?;
    assert_eq!(player_permissions.len(), 4);

    // Close the meadow to both actors by removing two edges at once.
    registry.remove_edges(cartesian_edges([PLAYER, GUIDE], CAN_ENTER, MEADOW))?;
    assert!(!registry.has_edge(PLAYER, CAN_ENTER, MEADOW));
    assert!(!registry.has_edge(GUIDE, CAN_ENTER, MEADOW));

    println!("added 8 Cartesian edges, queried 4, and removed 2");
    Ok(())
}
