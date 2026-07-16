use bevy_semantics::{cartesian_edges, EdgeRegistration, Kind, SemanticError, SemanticRegistry};

#[test]
fn cartesian_edges_accepts_scalar_and_collection_lanes() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let s1 = registry.register_kind("S1")?;
    let s2 = registry.register_kind("S2")?;
    let r1 = registry.register_kind("R1")?;
    let r2 = registry.register_kind("R2")?;
    let t1 = registry.register_kind("T1")?;
    let t2 = registry.register_kind("T2")?;

    assert_eq!(
        cartesian_edges([s1, s2], [r1, r2], [t1, t2]),
        vec![
            (s1, r1, t1),
            (s1, r1, t2),
            (s1, r2, t1),
            (s1, r2, t2),
            (s2, r1, t1),
            (s2, r1, t2),
            (s2, r2, t1),
            (s2, r2, t2),
        ]
    );
    assert_eq!(cartesian_edges(s1, r1, [t1, t2]).len(), 2);
    assert_eq!(cartesian_edges(s1, [r1, r2], t1).len(), 2);
    assert_eq!(cartesian_edges([s1, s2], r1, [t1, t2]).len(), 4);
    assert_eq!(cartesian_edges([s1, s2], r1, t1).len(), 2);
    assert_eq!(cartesian_edges([s1, s2], [r1, r2], t1).len(), 4);
    Ok(())
}

#[test]
fn cartesian_products_work_for_add_remove_and_query() -> Result<(), SemanticError> {
    let mut registry = SemanticRegistry::default();
    let s1 = registry.register_kind("S1")?;
    let s2 = registry.register_kind("S2")?;
    let r1 = registry.register_kind("R1")?;
    let r2 = registry.register_kind("R2")?;
    let t1 = registry.register_kind("T1")?;
    let t2 = registry.register_kind("T2")?;

    let edges = cartesian_edges([s1, s2], [r1, r2], [t1, t2]);
    assert!(registry.add_edges(&edges)?);

    let snapshot = registry.snapshot();
    assert_eq!(
        snapshot
            .edge_query()
            .cartesian([s1, s2], [r1, r2], [t1, t2])
            .run_count(&snapshot)?,
        8
    );
    let selected = snapshot
        .edge_query()
        .cartesian([s1, s2], r1, [t1, t2])
        .run_edges(&snapshot)?;
    assert_eq!(selected.len(), 4);
    assert!(selected.iter().all(|edge| edge.relation == r1));

    let empty: [Kind; 0] = [];
    assert_eq!(
        snapshot
            .edge_query()
            .cartesian(empty, r1, t1)
            .run_count(&snapshot)?,
        0
    );

    assert!(registry.remove_edges(cartesian_edges(s1, [r1, r2], t1))?);
    assert!(!registry.has_edge(s1, r1, t1));
    assert!(!registry.has_edge(s1, r2, t1));
    assert!(registry.has_edge(s2, r1, t1));

    registry.add_edge(s1, r1, t1)?;
    let unknown = Kind::from(u64::MAX);
    let removals: [EdgeRegistration; 2] = [(s1, r1, t1), (s1, r1, unknown)];
    assert!(matches!(
        registry.remove_edges(removals),
        Err(SemanticError::UnknownKind { kind }) if kind == unknown
    ));
    assert!(registry.has_edge(s1, r1, t1));
    Ok(())
}
