use bevy_semantics::core::{IS_A, RELATION};
use bevy_semantics::{cartesian_edges, semantic_kinds, SemanticError, SemanticRegistry};

semantic_kinds! {
    const EXAMPLE_KINDS = {
        WOLF = "Wolf",
        FOX = "Fox",
        LIKES = "likes",
        AVOIDS = "avoids",
        FOREST = "Forest",
        MEADOW = "Meadow",
    };
}

fn main() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    registry.register_consts(EXAMPLE_KINDS)?;
    registry.add_edges([(LIKES, IS_A, RELATION), (AVOIDS, IS_A, RELATION)])?;

    let combinations = cartesian_edges([WOLF, FOX], [LIKES, AVOIDS], [FOREST, MEADOW]);
    assert_eq!(combinations.len(), 8);
    registry.add_edges(combinations)?;

    let snapshot = registry.snapshot();
    let wolf_edges = snapshot
        .edge_query()
        .cartesian(WOLF, [LIKES, AVOIDS], [FOREST, MEADOW])
        .run_edges(&snapshot)?;
    assert_eq!(wolf_edges.len(), 4);

    registry.remove_edges(cartesian_edges([WOLF, FOX], AVOIDS, FOREST))?;
    assert!(!registry.has_edge(WOLF, AVOIDS, FOREST));
    assert!(!registry.has_edge(FOX, AVOIDS, FOREST));

    println!("added 8 Cartesian edges, queried 4, and removed 2");
    Ok(())
}
